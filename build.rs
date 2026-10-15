use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const INPUTS: [(&str, &str); 2] = [
    ("THIRD-PARTY-NOTICES.txt", "cogcalc-third-party-notices.txt"),
    ("SOURCE.txt", "cogcalc-source.txt"),
];

fn main() {
    println!("cargo:rerun-if-env-changed=COGCALC_LEGAL_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build output directory"));
    let explicit = env::var_os("COGCALC_LEGAL_DIR");
    let directory = match &explicit {
        Some(value) => {
            assert!(
                !value.is_empty(),
                "COGCALC_LEGAL_DIR must name a directory containing THIRD-PARTY-NOTICES.txt and SOURCE.txt"
            );
            // Resolve relative paths consistently, including when Cargo was
            // invoked outside the package directory with --manifest-path.
            manifest.join(value)
        }
        None => manifest,
    };

    for (name, _) in INPUTS {
        println!("cargo:rerun-if-changed={}", directory.join(name).display());
    }
    let required =
        explicit.is_some() || INPUTS.iter().any(|(name, _)| directory.join(name).exists());
    let texts = INPUTS.map(|(name, _)| {
        if required {
            read_notice(&directory.join(name))
        } else {
            // Ordinary development checkouts need no packaging prerequisites.
            // Release automation supplies the directory; source archives carry
            // both files at their root for offline rebuilds.
            String::new()
        }
    });
    if required {
        let version = env::var("CARGO_PKG_VERSION").expect("Cargo package version");
        let expected = format!("CogCalc v{version}");
        assert_eq!(
            texts[1].trim_start_matches('\u{feff}').lines().next(),
            Some(expected.as_str()),
            "SOURCE.txt must start with {expected:?}; regenerate the legal files for this release"
        );
    }
    for ((_, destination), text) in INPUTS.into_iter().zip(texts) {
        write_if_changed(&output.join(destination), text.as_bytes());
    }
}

fn read_notice(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "Could not read required UTF-8 legal file {}: {error}",
            path.display()
        )
    });
    assert!(
        !text
            .trim_matches(|character: char| character.is_whitespace() || character == '\u{feff}')
            .is_empty()
            && !text.contains('\0'),
        "Legal file {} must contain nonempty UTF-8 text without NUL bytes",
        path.display()
    );
    text
}

fn write_if_changed(path: &Path, contents: &[u8]) {
    if fs::read(path).ok().as_deref() != Some(contents) {
        fs::write(path, contents).unwrap_or_else(|error| {
            panic!("Could not embed legal file {}: {error}", path.display())
        });
    }
}
