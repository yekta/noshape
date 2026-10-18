//! Build script for `noshape-kernel`.
//!
//! Compiles the C++ bridge in `cpp/` with `cxx-build` (C++17) against a static,
//! headless OpenCascade (OCCT) installation located by the `OCCT_ROOT`
//! environment variable, and links the OCCT toolkits in dependency order
//! (dependents before dependencies), plus the platform system libraries that
//! OCCT needs.
//!
//! Produce an OCCT installation with `scripts/build-occt.sh` (Linux/macOS) or
//! `scripts/build-occt.ps1` (Windows).

use std::env;
use std::path::{Path, PathBuf};

/// OCCT toolkits we link, ordered so that every toolkit appears before the
/// toolkits it depends on (required for single-pass static linking with GNU ld).
/// Derived from `lib/cmake/opencascade/*Targets.cmake` INTERFACE_LINK_LIBRARIES.
const OCCT_LIBS: &[&str] = &[
    // Data exchange
    "TKDEOBJ",
    "TKDESTL",
    "TKRWMesh",
    "TKDESTEP",
    "TKDEIGES",
    "TKXCAF",
    "TKVCAF",
    "TKV3d",
    "TKService",
    "TKCAF",
    "TKLCAF",
    "TKCDF",
    "TKXSBase",
    "TKDE",
    // Modeling algorithms
    "TKFeat",
    "TKOffset",
    "TKFillet",
    "TKBool",
    "TKBO",
    "TKHLR",
    "TKHelix",
    "TKMesh",
    "TKShHealing",
    "TKPrim",
    "TKTopAlgo",
    "TKGeomAlgo",
    // Modeling data
    "TKBRep",
    "TKGeomBase",
    "TKG3d",
    "TKG2d",
    // Foundation
    "TKMath",
    "TKernel",
];

const BRIDGE_SOURCES: &[&str] = &[
    "cpp/common.cpp",
    "cpp/geom.cpp",
    "cpp/build.cpp",
    "cpp/features.cpp",
    "cpp/boolean.cpp",
    "cpp/mesh.cpp",
    "cpp/io.cpp",
    "cpp/mesh_io.cpp",
    "cpp/hlr.cpp",
    "cpp/surfacing.cpp",
];

fn main() {
    println!("cargo:rerun-if-env-changed=OCCT_ROOT");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=cpp/noshape_occt.hpp");
    for s in BRIDGE_SOURCES {
        println!("cargo:rerun-if-changed={s}");
    }

    let occt_root = match env::var_os("OCCT_ROOT") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => panic!(
            "\n\nnoshape-kernel: the OCCT_ROOT environment variable is not set.\n\
             Build a static, headless OpenCascade 8 with `scripts/build-occt.sh <prefix>` \
             (or `scripts/build-occt.ps1 <prefix>` on Windows) and then build with \
             OCCT_ROOT=<prefix>.\nSee docs/BUILDING.md.\n\n"
        ),
    };
    let include_dir = find_include_dir(&occt_root);
    let lib_dir = find_lib_dir(&occt_root);

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let msvc = target_env == "msvc";

    let mut build = cxx_build::bridge("src/ffi.rs");
    build
        .files(BRIDGE_SOURCES)
        .include("cpp")
        .include(&include_dir)
        .std("c++17")
        .define("OCCT_STATIC_BUILD", None)
        .define("HAVE_NO_DLL", None);
    if msvc {
        build
            .flag("/EHsc")
            .flag("/bigobj")
            .flag("/utf-8")
            .flag("/wd4996") // deprecated declarations (OCCT 8 deprecates many typedefs)
            .define("WNT", None)
            .define("_USE_MATH_DEFINES", None)
            .define("NOMINMAX", None);
    } else {
        build
            .flag_if_supported("-Wno-deprecated-declarations")
            .flag_if_supported("-Wno-unused-parameter")
            .flag_if_supported("-Wno-ignored-qualifiers")
            .flag_if_supported("-Wno-unused-variable")
            .flag_if_supported("-Wno-missing-field-initializers")
            .flag_if_supported("-Wno-deprecated-copy")
            .flag_if_supported("-Wno-cpp");
    }
    // The bridge is hot code (tessellation, history); always optimize it.
    build.opt_level(2);
    build.compile("noshape_occt");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    for lib in OCCT_LIBS {
        // `-bundle`: do not copy the (large) OCCT archives into our rlib; the
        // final link of the downstream binary references them directly.
        println!("cargo:rustc-link-lib=static:-bundle={lib}");
    }

    match target_os.as_str() {
        "linux" | "freebsd" | "netbsd" | "openbsd" | "dragonfly" => {
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=dl");
            println!("cargo:rustc-link-lib=dylib=m");
            if target_os == "linux" {
                println!("cargo:rustc-link-lib=dylib=rt");
            }
        }
        "macos" | "ios" => {
            // TKService (pulled in by XCAF) references Objective-C/AppKit/IOKit
            // symbols on Apple platforms. The C++ standard library (libc++) is
            // linked by cxx-build.
            println!("cargo:rustc-link-lib=dylib=objc");
            println!("cargo:rustc-link-lib=framework=Foundation");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
            println!("cargo:rustc-link-lib=framework=AppKit");
            println!("cargo:rustc-link-lib=framework=IOKit");
        }
        "windows" => {
            for l in [
                "advapi32", "gdi32", "user32", "shell32", "wsock32", "ws2_32", "psapi",
                "winmm", "ole32",
            ] {
                println!("cargo:rustc-link-lib=dylib={l}");
            }
        }
        _ => {}
    }
}

fn find_include_dir(root: &Path) -> PathBuf {
    for cand in [
        root.join("include").join("opencascade"),
        root.join("inc"),
        root.join("include"),
    ] {
        if cand.join("Standard.hxx").exists() {
            return cand;
        }
    }
    panic!(
        "noshape-kernel: could not find OCCT headers (Standard.hxx) under {}/include/opencascade. \
         Is OCCT_ROOT pointing at an OCCT install prefix produced by scripts/build-occt.sh?",
        root.display()
    );
}

fn find_lib_dir(root: &Path) -> PathBuf {
    let candidates = [
        root.join("lib"),
        root.join("win64").join("vc14").join("lib"),
        root.join("lib64"),
    ];
    for cand in &candidates {
        if cand.join("libTKernel.a").exists() || cand.join("TKernel.lib").exists() {
            return cand.clone();
        }
    }
    panic!(
        "noshape-kernel: could not find static OCCT libraries (libTKernel.a / TKernel.lib) under {}/lib. \
         Build OCCT with scripts/build-occt.sh (BUILD_LIBRARY_TYPE=Static).",
        root.display()
    );
}
