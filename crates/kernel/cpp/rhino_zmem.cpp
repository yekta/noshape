// zlib memory hooks for openNURBS' zlib (built with MY_ZCALLOC). openNURBS
// defines them itself, but its zlib needs them after it in a static link;
// these sit after zlib (build.rs). Same behaviour: calloc / free.
#include <cstdlib>

extern "C" void* zcalloc(void* opaque, unsigned items, unsigned size)
{
  (void)opaque;
  return std::calloc(items, size);
}

extern "C" void zcfree(void* opaque, void* ptr)
{
  (void)opaque;
  std::free(ptr);
}
