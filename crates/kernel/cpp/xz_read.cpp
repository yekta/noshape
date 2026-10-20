// XZ (LZMA2) decompression, as JT 10 compresses its segments; written from
// the .xz and LZMA specifications, standard library only. Single-filter
// LZMA2 blocks (what JT writes); checks are skipped. Every read and every
// back-reference is bounds-checked: the input is untrusted.

#include "internal.hpp"

#include <array>
#include <cstring>

namespace noshape {

namespace {

[[noreturn]] void bad(const char* what)
{
  fail_invalid(std::string("xz: ") + what);
}

constexpr size_t kMaxOutput = size_t(1) << 31; // 2 GB

struct Lzma
{
  // Input (one LZMA2 chunk's compressed bytes at a time).
  const unsigned char* in  = nullptr;
  size_t               len = 0, pos = 0;
  uint32_t             range = 0, code = 0;
  // Output = the dictionary.
  std::string& out;
  // Properties and state.
  int      lc = 3, lp = 0, pb = 2;
  int      state = 0;
  uint32_t rep[4] = {0, 0, 0, 0};
  // Probabilities.
  std::array<uint16_t, 192> is_match, is_rep0_long;
  std::array<uint16_t, 12>  is_rep, is_rep_g0, is_rep_g1, is_rep_g2;
  std::array<uint16_t, 256> pos_slot;  // 4 x 64
  std::array<uint16_t, 115> spec_pos;  // distances 4..127
  std::array<uint16_t, 16>  align;
  struct Len
  {
    uint16_t                  choice, choice2;
    std::array<uint16_t, 128> low, mid; // 16 x 8
    std::array<uint16_t, 256> high;
  } len_dec, rep_len_dec;
  std::vector<uint16_t> literal;

  explicit Lzma(std::string& o) : out(o) {}

  void reset_state()
  {
    state = 0;
    std::memset(rep, 0, sizeof rep);
    is_match.fill(1024), is_rep0_long.fill(1024), is_rep.fill(1024), is_rep_g0.fill(1024), is_rep_g1.fill(1024), is_rep_g2.fill(1024);
    pos_slot.fill(1024), spec_pos.fill(1024), align.fill(1024);
    for (Len* l : {&len_dec, &rep_len_dec})
    {
      l->choice = l->choice2 = 1024;
      l->low.fill(1024), l->mid.fill(1024), l->high.fill(1024);
    }
    literal.assign((size_t)0x300 << (lc + lp), 1024);
  }
  void set_props(int b)
  {
    if (b >= 9 * 5 * 5)
      bad("bad LZMA properties");
    lc = b % 9;
    b /= 9;
    lp = b % 5;
    pb = b / 5;
    if (lc + lp > 4)
      bad("bad LZMA2 properties");
  }

  unsigned char next()
  {
    if (pos >= len)
      bad("truncated LZMA data");
    return in[pos++];
  }
  void init_range()
  {
    if (next() != 0)
      bad("bad range coder start");
    range = 0xFFFFFFFF;
    code  = 0;
    for (int i = 0; i < 4; ++i)
      code = (code << 8) | next();
    if (code == range)
      bad("bad range coder start");
  }
  void normalize()
  {
    if (range < (1u << 24))
    {
      range <<= 8;
      code = (code << 8) | next();
    }
  }
  int bit(uint16_t& p)
  {
    const uint32_t bound = (range >> 11) * p;
    int            b;
    if (code < bound)
    {
      range = bound;
      p     = (uint16_t)(p + ((2048 - p) >> 5));
      b     = 0;
    }
    else
    {
      range -= bound;
      code -= bound;
      p = (uint16_t)(p - (p >> 5));
      b = 1;
    }
    normalize();
    return b;
  }
  uint32_t direct(int n)
  {
    uint32_t r = 0;
    for (int i = 0; i < n; ++i)
    {
      range >>= 1;
      uint32_t b = 0;
      if (code >= range)
      {
        code -= range;
        b = 1;
      }
      r = (r << 1) | b;
      normalize();
    }
    return r;
  }
  uint32_t tree(uint16_t* p, int bits)
  {
    uint32_t m = 1;
    for (int i = 0; i < bits; ++i)
      m = (m << 1) | (uint32_t)bit(p[m]);
    return m - (1u << bits);
  }
  uint32_t reverse_tree(uint16_t* p, int bits)
  {
    uint32_t m = 1, s = 0;
    for (int i = 0; i < bits; ++i)
    {
      const uint32_t b = (uint32_t)bit(p[m]);
      m                = (m << 1) | b;
      s |= b << i;
    }
    return s;
  }
  uint32_t length(Len& l, uint32_t ps)
  {
    if (!bit(l.choice))
      return tree(&l.low[ps * 8], 3);
    if (!bit(l.choice2))
      return 8 + tree(&l.mid[ps * 8], 3);
    return 16 + tree(l.high.data(), 8);
  }
  uint32_t distance(uint32_t len)
  {
    const uint32_t ls   = std::min<uint32_t>(len, 3);
    const uint32_t slot = tree(&pos_slot[ls * 64], 6);
    if (slot < 4)
      return slot;
    const int n    = (int)(slot >> 1) - 1;
    uint32_t  dist = (2 | (slot & 1)) << n;
    if (slot < 14)
    {
      // Reverse tree over spec_pos, based so that its first node is at dist - slot.
      const int64_t base = (int64_t)dist - (int64_t)slot - 1;
      if (base < -1 || base + (1 << n) >= (int64_t)spec_pos.size())
        bad("bad distance");
      uint32_t m = 1, s = 0;
      for (int i = 0; i < n; ++i)
      {
        const int64_t k = base + (int64_t)m;
        if (k < 0 || k >= (int64_t)spec_pos.size())
          bad("bad distance");
        const uint32_t b = (uint32_t)bit(spec_pos[(size_t)k]);
        m                = (m << 1) | b;
        s |= b << i;
      }
      return dist + s;
    }
    dist += direct(n - 4) << 4;
    return dist + reverse_tree(align.data(), 4);
  }

  void put(unsigned char c)
  {
    if (out.size() >= kMaxOutput)
      bad("output too large");
    out.push_back((char)c);
  }
  void copy(uint32_t dist, uint32_t n, size_t dict_start)
  {
    if ((size_t)dist + 1 > out.size() - dict_start)
      bad("distance beyond the dictionary");
    for (uint32_t i = 0; i < n; ++i)
      put((unsigned char)out[out.size() - dist - 1]);
  }

  // Decode until `target` bytes of output (an LZMA2 chunk's size).
  void run(size_t target, size_t dict_start)
  {
    while (out.size() < target)
    {
      const uint32_t ps = (uint32_t)(out.size() & ((1u << pb) - 1));
      if (!bit(is_match[(size_t)(state << 4) + ps]))
      {
        const unsigned char prev = out.size() > dict_start ? (unsigned char)out.back() : 0;
        const uint32_t      ls   = (uint32_t)(((out.size() & ((1u << lp) - 1)) << lc) + (prev >> (8 - lc)));
        uint16_t*           p    = &literal[(size_t)0x300 * ls];
        uint32_t            sym  = 1;
        if (state >= 7)
        {
          if ((size_t)rep[0] + 1 > out.size() - dict_start)
            bad("distance beyond the dictionary");
          uint32_t mb = (unsigned char)out[out.size() - rep[0] - 1];
          while (sym < 0x100)
          {
            const uint32_t mbit = (mb >> 7) & 1;
            mb <<= 1;
            const uint32_t b = (uint32_t)bit(p[0x100 + (mbit << 8) + sym]);
            sym              = (sym << 1) | b;
            if (mbit != b)
              break;
          }
        }
        while (sym < 0x100)
          sym = (sym << 1) | (uint32_t)bit(p[sym]);
        put((unsigned char)sym);
        state = state < 4 ? 0 : state < 10 ? state - 3 : state - 6;
        continue;
      }
      uint32_t n;
      if (!bit(is_rep[(size_t)state]))
      {
        rep[3] = rep[2], rep[2] = rep[1], rep[1] = rep[0];
        n      = length(len_dec, ps);
        state  = state < 7 ? 7 : 10;
        rep[0] = distance(n);
        if (rep[0] == 0xFFFFFFFF)
          bad("unexpected end marker");
      }
      else
      {
        if (!bit(is_rep_g0[(size_t)state]))
        {
          if (!bit(is_rep0_long[(size_t)(state << 4) + ps]))
          {
            state = state < 7 ? 9 : 11;
            copy(rep[0], 1, dict_start);
            continue;
          }
        }
        else
        {
          uint32_t d;
          if (!bit(is_rep_g1[(size_t)state]))
            d = rep[1];
          else
          {
            if (!bit(is_rep_g2[(size_t)state]))
              d = rep[2];
            else
            {
              d      = rep[3];
              rep[3] = rep[2];
            }
            rep[2] = rep[1];
          }
          rep[1] = rep[0];
          rep[0] = d;
        }
        n     = length(rep_len_dec, ps);
        state = state < 7 ? 8 : 11;
      }
      copy(rep[0], std::min<uint32_t>(n + 2, (uint32_t)(target - out.size())), dict_start);
    }
  }
};

uint64_t varint(const unsigned char* p, size_t len, size_t& at)
{
  uint64_t v = 0;
  for (int s = 0; s < 63; s += 7)
  {
    if (at >= len)
      bad("truncated header");
    const unsigned char b = p[at++];
    v |= (uint64_t)(b & 0x7f) << s;
    if (!(b & 0x80))
      return v;
  }
  bad("bad number");
}

} // namespace

std::string xz_decompress(const unsigned char* in, size_t len)
{
  static const unsigned char magic[6] = {0xFD, '7', 'z', 'X', 'Z', 0};
  if (len < 12 + 12 || std::memcmp(in, magic, 6) != 0)
    bad("not an xz stream");
  const int    check      = in[7] & 0x0f;
  const size_t check_size = check == 0 ? 0 : check <= 3 ? 4 : check <= 6 ? 8 : check <= 9 ? 16 : check <= 12 ? 32 : 64;
  std::string  out;
  Lzma         z(out);
  size_t       at = 12;
  for (;;)
  {
    if (at >= len)
      bad("truncated stream");
    if (in[at] == 0)
      break; // the index: no more blocks
    const size_t hsize = ((size_t)in[at] + 1) * 4;
    if (at + hsize > len)
      bad("truncated block header");
    size_t      h     = at + 2;
    const int   flags = in[at + 1];
    if ((flags & 3) != 0)
      bad("filter chains are not supported");
    if (flags & 0x40)
      varint(in, at + hsize, h); // compressed size
    if (flags & 0x80)
      varint(in, at + hsize, h); // uncompressed size
    if (varint(in, at + hsize, h) != 0x21)
      bad("only LZMA2 is supported");
    if (varint(in, at + hsize, h) != 1)
      bad("bad LZMA2 filter properties");
    at += hsize;
    // LZMA2 chunks.
    size_t dict_start = out.size();
    bool   need_props = true;
    for (;;)
    {
      if (at >= len)
        bad("truncated LZMA2 data");
      const int c = in[at++];
      if (c == 0)
        break;
      if (c == 1 || c == 2)
      {
        if (at + 2 > len)
          bad("truncated chunk");
        const size_t n = (((size_t)in[at] << 8) | in[at + 1]) + 1;
        at += 2;
        if (at + n > len || out.size() + n > kMaxOutput)
          bad("truncated chunk");
        if (c == 1)
          dict_start = out.size();
        out.append((const char*)in + at, n);
        at += n;
        continue;
      }
      if (c < 0x80)
        bad("bad LZMA2 chunk");
      if (at + 4 > len)
        bad("truncated chunk");
      const size_t unpacked = (((size_t)(c & 0x1f) << 16) | ((size_t)in[at] << 8) | in[at + 1]) + 1;
      const size_t packed   = (((size_t)in[at + 2] << 8) | in[at + 3]) + 1;
      at += 4;
      const int reset = (c >> 5) & 3;
      if (reset == 3)
        dict_start = out.size();
      if (reset >= 2)
      {
        if (at >= len)
          bad("truncated chunk");
        z.set_props(in[at++]);
        need_props = false;
      }
      else if (need_props)
        bad("LZMA2 chunk without properties");
      if (reset >= 1)
        z.reset_state();
      if (at + packed > len)
        bad("truncated chunk");
      z.in  = in + at;
      z.len = packed;
      z.pos = 0;
      z.init_range();
      z.run(out.size() + unpacked, dict_start);
      at += packed;
    }
    // Block padding, then the check.
    while (at % 4 != 0)
      ++at;
    at += check_size;
  }
  return out;
}

} // namespace noshape
