{
  description = "Observatory";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    fenix,
    flake-utils,
    advisory-db,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};

      inherit (pkgs) lib;

      craneLib = crane.lib.${system};
      src = lib.cleanSourceWith {
        src = craneLib.path ./.; # The original, unfiltered source
        filter = path: type:
          (builtins.match ".*static.*$" path != null) || (craneLib.filterCargoSources path type);
      };

      # Common arguments can be set here to avoid repeating them later
      commonArgs = {
        inherit src;
        strictDeps = true;

        buildInputs = [
          pkgs.sqlite
        ];

        # Additional environment variables can be set directly
        # MY_CUSTOM_VAR = "some value";
      };

      craneLibLLvmTools =
        craneLib.overrideToolchain
        (fenix.packages.${system}.complete.withComponents [
          "cargo"
          # "llvm-tools"
          "rustc"
        ]);

      # Build *just* the cargo dependencies, so we can reuse
      # all of that work (e.g. via cachix) when running in CI
      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      # Build the actual crate itself, reusing the dependency
      # artifacts from above.
      observatory = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
          postInstall = ''
            mkdir -p $out/static
            cp -r ${src}/static/* $out/static/
          '';
        });
    in {
      checks = {
        # Build the crate as part of `nix flake check` for convenience
        inherit observatory;

        # Run clippy (and deny all warnings) on the crate source,
        # again, reusing the dependency artifacts from above.
        #
        # Note that this is done as a separate derivation so that
        # we can block the CI if there are issues here, but not
        # prevent downstream consumers from building our crate by itself.
        observatory-clippy = craneLib.cargoClippy (commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

        observatory-doc = craneLib.cargoDoc (commonArgs
          // {
            inherit cargoArtifacts;
          });

        # Check formatting
        observatory-fmt = craneLib.cargoFmt {
          inherit src;
        };

        # Audit dependencies
        observatory-audit = craneLib.cargoAudit {
          inherit src advisory-db;
        };

        # Audit licenses
        observatory-deny = craneLib.cargoDeny {
          inherit src;
        };

        # Run tests with cargo-nextest
        # Consider setting `doCheck = false` on `observatory` if you do not want
        # the tests to run twice
        observatory-nextest = craneLib.cargoNextest (commonArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });
      };

      packages = {
        inherit observatory;
        default = observatory;
      };

      apps = {
        observatory = flake-utils.lib.mkApp {
          drv = observatory;
        };
        default = flake-utils.lib.mkApp {
          drv = observatory;
        };
      };

      devShells.default = craneLib.devShell {
        # Inherit inputs from checks.
        checks = self.checks.${system};

        packages = [
        ];
      };
    });
}
