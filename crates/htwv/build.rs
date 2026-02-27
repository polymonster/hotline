fn main() {
    #[cfg(target_os = "macos")]
    macos_build();
}

#[cfg(target_os = "macos")]
fn macos_build() {
    use std::path::Path;
    use std::process::{Command, Stdio};

    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    let spirv_cross_src = format!("{}/third_party/SPIRV-Cross", manifest_dir);
    let pmfx_shader_src = format!("{}/third_party/pmfx-shader", manifest_dir);

    // Ensure third-party dependencies exist (download if missing for crates.io)
    ensure_spirv_cross(&spirv_cross_src);
    ensure_pmfx_shader(&pmfx_shader_src);

    // Rerun if SPIRV-Cross source changes
    println!("cargo:rerun-if-changed={}/spirv_cross_c.h", spirv_cross_src);

    // Select build profile
    let profile = if std::env::var("CARGO_FEATURE_CPP_RELEASE").is_ok() {
        "Release"
    } else {
        match std::env::var("PROFILE")
            .unwrap_or_default()
            .as_str()
        {
            "debug" => "Debug",
            _ => "Release",
        }
    };

    // Build SPIRV-Cross in OUT_DIR
    let build_dir = format!("{}/SPIRV-Cross", out_dir);
    std::fs::create_dir_all(&build_dir).unwrap();

    // Configure CMake
    let status = Command::new("cmake")
        .args([
            "-S",
            &spirv_cross_src,
            "-B",
            &build_dir,
            &format!("-DCMAKE_BUILD_TYPE={}", profile),
            "-DCMAKE_OSX_DEPLOYMENT_TARGET=15.0",
            "-DCMAKE_CXX_COMPILER=clang++",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to run cmake configure");

    if !status.success() {
        panic!("CMake configure failed");
    }

    // Build
    let status = Command::new("cmake")
        .args(["--build", &build_dir, "--config", profile])
        .status()
        .expect("Failed to run cmake build");

    if !status.success() {
        panic!("CMake build failed");
    }

    // Optionally regenerate bindings
    if std::env::var("CARGO_FEATURE_GENERATE_BINDINGS").is_ok() {
        let bindings = bindgen::Builder::default()
            .header(format!("{}/spirv_cross_c.h", spirv_cross_src))
            .generate()
            .expect("Failed to generate bindings for spirv_cross_c.h");

        bindings
            .write_to_file(format!("{}/src/spirv_cross_bindings.rs", manifest_dir))
            .expect("Couldn't write bindings!");
    }

    // Setup link paths
    println!("cargo:rustc-link-search=native={}", build_dir);
    println!("cargo:rustc-link-lib=static=spirv-cross-c");
    println!("cargo:rustc-link-lib=static=spirv-cross-core");
    println!("cargo:rustc-link-lib=static=spirv-cross-cpp");
    println!("cargo:rustc-link-lib=static=spirv-cross-glsl");
    println!("cargo:rustc-link-lib=static=spirv-cross-hlsl");
    println!("cargo:rustc-link-lib=static=spirv-cross-msl");
    println!("cargo:rustc-link-lib=static=spirv-cross-reflect");
    println!("cargo:rustc-link-lib=static=spirv-cross-util");
    println!("cargo:rustc-link-lib=c++");
}

#[cfg(target_os = "macos")]
fn ensure_spirv_cross(spirv_cross_dir: &str) {
    use std::path::Path;
    use std::process::{Command, Stdio};

    let marker = Path::new(spirv_cross_dir).join("CMakeLists.txt");
    if marker.exists() {
        return; // Already populated (submodule or previous download)
    }

    println!("cargo:warning=SPIRV-Cross not found, downloading...");

    // Pin to a specific release for reproducibility
    const SPIRV_CROSS_VERSION: &str = "vulkan-sdk-1.3.275.0";
    let url = format!(
        "https://github.com/KhronosGroup/SPIRV-Cross/archive/refs/tags/{}.tar.gz",
        SPIRV_CROSS_VERSION
    );

    let parent = Path::new(spirv_cross_dir)
        .parent()
        .expect("Invalid spirv_cross_dir");
    std::fs::create_dir_all(parent).expect("Failed to create third_party dir");

    // Download and extract
    let status = Command::new("curl")
        .args(["-L", "-o", "/tmp/spirv-cross.tar.gz", &url])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to download SPIRV-Cross");

    if !status.success() {
        panic!("Failed to download SPIRV-Cross from {}", url);
    }

    let status = Command::new("tar")
        .args([
            "-xzf",
            "/tmp/spirv-cross.tar.gz",
            "-C",
            parent.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to extract SPIRV-Cross");

    if !status.success() {
        panic!("Failed to extract SPIRV-Cross");
    }

    // Rename extracted directory
    let extracted_name = format!("SPIRV-Cross-{}", SPIRV_CROSS_VERSION);
    let extracted_path = parent.join(&extracted_name);
    std::fs::rename(&extracted_path, spirv_cross_dir)
        .expect("Failed to rename extracted SPIRV-Cross directory");

    println!("cargo:warning=SPIRV-Cross downloaded successfully");
}

#[cfg(target_os = "macos")]
fn ensure_pmfx_shader(pmfx_shader_dir: &str) {
    use std::path::Path;
    use std::process::{Command, Stdio};

    let marker = Path::new(pmfx_shader_dir).join("pmfx.py");
    if marker.exists() {
        return; // Already populated
    }

    println!("cargo:warning=pmfx-shader not found, downloading...");

    // Pin to a specific commit/tag for reproducibility
    const PMFX_SHADER_REF: &str = "master"; // TODO: pin to specific tag/commit
    let url = format!(
        "https://github.com/polymonster/pmfx-shader/archive/refs/heads/{}.tar.gz",
        PMFX_SHADER_REF
    );

    let parent = Path::new(pmfx_shader_dir)
        .parent()
        .expect("Invalid pmfx_shader_dir");
    std::fs::create_dir_all(parent).expect("Failed to create third_party dir");

    // Download and extract
    let status = Command::new("curl")
        .args(["-L", "-o", "/tmp/pmfx-shader.tar.gz", &url])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to download pmfx-shader");

    if !status.success() {
        panic!("Failed to download pmfx-shader from {}", url);
    }

    let status = Command::new("tar")
        .args([
            "-xzf",
            "/tmp/pmfx-shader.tar.gz",
            "-C",
            parent.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to extract pmfx-shader");

    if !status.success() {
        panic!("Failed to extract pmfx-shader");
    }

    // Rename extracted directory
    let extracted_name = format!("pmfx-shader-{}", PMFX_SHADER_REF);
    let extracted_path = parent.join(&extracted_name);
    std::fs::rename(&extracted_path, pmfx_shader_dir)
        .expect("Failed to rename extracted pmfx-shader directory");

    println!("cargo:warning=pmfx-shader downloaded successfully");
}
