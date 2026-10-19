// JT 9.5 file writer (self-contained, C++17 standard library only).
//
// Port of the verified Python prototype writer2.py (+ topoenc.py, enc.py, bl2.py,
// meshtest.close_mesh).  The output layout follows the files written by Onshape:
//
//   header (105 bytes) | TOC | LSG segment (type 1, zlib)
//   | per part: Tri-Strip Shape LOD segment (type 7, raw)
//   |           [XT B-Rep segment (type 17, zlib) when Part::xt is non-empty]
//
// Shape LOD data uses the JT "topologically compressed" representation: the triangle
// mesh (closed with one cover polygon per boundary loop) is encoded by running the
// Appendix-E decoder state machine against the source mesh.  Inputs the connected
// encoder cannot handle (non-manifold edges/vertices, degenerate triangles) fall back
// to one closed "pillow" (triangle + reversed cover) per triangle.
//
// Every integer array is a CDP2 packet with the fixed-width bit-length codec;
// coordinates and normals are stored losslessly (float32 bit patterns).
#pragma once

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace noshape {
namespace jt {

struct Part {
  std::string name;
  std::vector<std::array<double, 3>> positions;      // mm
  std::vector<std::array<int, 3>> tris;              // CCW from outside
  std::vector<int> groups;                            // per triangle: B-rep face index
  std::vector<std::array<std::array<float, 3>, 3>> corner_normals; // per triangle corner
  std::string xt;                                     // Parasolid neutral binary bytes, may be empty
  std::array<int, 3> xt_version{37, 1, 212};          // modeller version of `xt` (major, minor, build)
  std::array<float, 3> rgb{0.615686f, 0.811765f, 0.929412f};
};

// Encodes `parts` as a JT 9.5 file and returns the file bytes.
//
// `file_name` is stored in the partition node (directories are stripped).  Segment
// GUIDs are random unless `fixed_guids` is given: GUID number k (k = 0, 1, ... in the
// order shape seg part 0, [XT seg part 0], shape seg part 1, ..., LSG seg) is then the
// base with k added to its little-endian uint32 at bytes 12..15.
//
// `part_modes`, when non-null, receives per part "connected" or "pillow(<reason>)".
//
// Throws std::invalid_argument for unusable input (no parts, a part without
// triangles, mismatched array sizes, vertex indices out of range).
std::string write(const std::vector<Part>& parts, const std::string& file_name,
                  const std::array<uint8_t, 16>* fixed_guids = nullptr,
                  std::vector<std::string>* part_modes = nullptr);

} // namespace jt
} // namespace noshape
