// PTC Creo View .pvz reader: the part instances' triangle meshes (format
// notes: /root/kern-docs/pvz/format-notes.md, outside the repo).  Onshape
// imports a PVZ as meshes only, so only the tessellation section of each
// part (OL section 1) is read, not its exact B-rep.  C++17, standard library
// only (zip and gzip are inflated here).
#pragma once

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace noshape {
namespace pvz {

// One placed part: its mesh in the part's coordinates (metres), and the
// placement (row-major 3x4: p' = R p + t) from the product tree.
struct MeshInstance
{
  std::string                  name;       // the instance node's name
  std::string                  ol_file;    // the part's zip member
  std::vector<double>          positions;  // x y z per vertex (faces' vertices back to back)
  std::vector<uint32_t>        triangles;  // 3 indices per triangle, counter-clockwise from outside
  std::array<double, 12>       placement{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0};
};

// Every part instance of the file (root last, children in file order).
// Throws std::runtime_error on a malformed file.
std::vector<MeshInstance> read_meshes(const std::string& bytes);

// Raw deflate (RFC 1951) of `in`; throws std::runtime_error.
std::string inflate_raw(const unsigned char* in, size_t len);

} // namespace pvz
} // namespace noshape
