{
  description = "Claudius - MCP Servers configuration management tool for Claude";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    pre-commit-hooks = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # Binary cache configuration for faster builds
  nixConfig = {
    substituters = [
      "https://cache.nixos.org"
      "https://nix-community.cachix.org"
    ];
    trusted-public-keys = [
      "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      pre-commit-hooks,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Use a specific Rust version for reproducibility.
        # Keep this in sync with the MSRV in Cargo.toml.
        rustToolchain = pkgs.rust-bin.stable."1.92.0".default;

        # Pre-commit hooks configuration
        # Note: Cargo-based hooks (rustfmt, clippy, audit, deny, machete) are disabled
        # because they require network access to download crate dependencies, which is
        # not available in the sandboxed Nix build environment during `nix flake check`.
        #
        # Developers can still run these manually:
        # - cargo fmt -- --check
        # - cargo clippy --all-targets --all-features -- -D warnings
        # - cargo audit
        # - cargo deny check
        # - cargo machete
        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            # Rust formatting
            rustfmt = {
              # Disabled in CI due to network access requirements
              enable = false;
              entry = "${rustToolchain}/bin/cargo fmt -- --check";
              types = [ "rust" ];
              pass_filenames = false;
            };

            # Clippy linting (uses clippy.toml and .cargo/config.toml for configuration)
            clippy = {
              # Disabled in CI due to network access requirements
              enable = false;
              entry = "${rustToolchain}/bin/cargo clippy --all-targets --all-features -- -D warnings";
              types = [ "rust" ];
              pass_filenames = false;
            };

            # Check for debug prints
            debug-statements = {
              enable = true;
              name = "Check for debug statements";
              entry = "${pkgs.writeShellScript "check-debug" ''
                # Exclude files with legitimate user output (interactive prompts and dry-run display)
                ! ${pkgs.ripgrep}/bin/rg --type rust -n 'dbg!|println!|eprintln!|print!|eprint!' \
                  --glob '!tests/**' \
                  --glob '!src/main.rs' \
                  --glob '!src/validation.rs' \
                  --glob '!src/merge.rs' \
                  --glob '!src/sync_operations.rs' \
                  --glob '!src/bootstrap.rs' \
                  . || (echo "Error: Debug statements found in non-test code" && exit 1)
              ''}";
              types = [ "rust" ];
              pass_filenames = false;
            };

            # Check for unwrap usage
            unwrap-check = {
              enable = true;
              name = "Check for unwrap usage";
              entry = "${pkgs.writeShellScript "check-unwrap" ''
                ! ${pkgs.ripgrep}/bin/rg --type rust -n '\.unwrap\(\)' --glob '!tests/**' . || (echo "Error: unwrap() usage found in non-test code" && exit 1)
              ''}";
              types = [ "rust" ];
              pass_filenames = false;
            };

            # Cargo.toml formatting
            cargo-toml-fmt = {
              enable = true;
              name = "Format Cargo.toml";
              entry = "${pkgs.taplo}/bin/taplo fmt";
              files = "Cargo\\.toml$";
              pass_filenames = true;
            };

            # Check TODO/FIXME comments
            todo-check = {
              enable = true;
              name = "Check for TODO/FIXME";
              entry = "${pkgs.writeShellScript "check-todo" ''
                ${pkgs.ripgrep}/bin/rg --type rust -n 'TODO|FIXME|HACK|XXX' . || exit 0
              ''}";
              types = [ "rust" ];
              pass_filenames = false;
            };

            # License headers check
            license-check = {
              enable = true;
              name = "Check license headers";
              entry = "${pkgs.writeShellScript "check-license" ''
                for file in $(find src -name "*.rs" -type f); do
                  if ! head -n 5 "$file" | grep -q "SPDX-License-Identifier" && ! head -n 5 "$file" | grep -q "Copyright"; then
                    echo "Missing license header in: $file"
                  fi
                done
              ''}";
              types = [ "rust" ];
              pass_filenames = false;
            };

            # Security audit
            cargo-audit = {
              # Disabled in CI due to network access requirements
              enable = false;
              name = "Security audit";
              entry = "${pkgs.cargo-audit}/bin/cargo-audit audit";
              pass_filenames = false;
              types = [ "rust" ];
            };

            # Dependency and license checking
            cargo-deny = {
              # Disabled in CI due to network access requirements
              enable = false;
              name = "Check dependencies and licenses";
              entry = "${pkgs.cargo-deny}/bin/cargo-deny check";
              pass_filenames = false;
              types = [ "rust" ];
            };

            # Unused dependency detection
            cargo-machete = {
              # Disabled in CI due to network access requirements
              enable = false;
              name = "Check for unused dependencies";
              entry = "${pkgs.cargo-machete}/bin/cargo-machete";
              pass_filenames = false;
              types = [ "rust" ];
            };

            # Spell check
            typos = {
              enable = true;
              name = "Spell check";
              entry = "${pkgs.typos}/bin/typos";
              pass_filenames = false;
            };

            # Markdown linting
            markdownlint = {
              enable = true;
              entry = "${pkgs.markdownlint-cli}/bin/markdownlint";
              types = [ "markdown" ];
              args = [
                "--disable"
                "MD013"
                "MD024"
                "MD025"
                "MD033"
              ];
              pass_filenames = true;
            };

            # Shell script checks
            shellcheck = {
              enable = true;
              entry = "${pkgs.shellcheck}/bin/shellcheck";
              types = [ "shell" ];
              pass_filenames = true;
            };

            # Nix formatting (RFC 166 style)
            nixfmt-rfc-style = {
              enable = true;
              entry = "${pkgs.nixfmt-rfc-style}/bin/nixfmt";
              types = [ "nix" ];
              pass_filenames = true;
            };

            # Nix linting with statix
            statix = {
              enable = true;
              name = "Nix static analysis";
              entry = "${pkgs.statix}/bin/statix check";
              types = [ "nix" ];
              pass_filenames = true;
            };

            # Dead code detection in Nix
            deadnix = {
              enable = true;
              name = "Find unused Nix code";
              entry = "${pkgs.deadnix}/bin/deadnix --fail";
              types = [ "nix" ];
              pass_filenames = true;
            };

            # Just formatting check
            just-check = {
              enable = true;
              name = "Check justfile formatting";
              entry = "${pkgs.writeShellScript "just-check" ''
                ${pkgs.just}/bin/just --unstable --fmt --check --justfile "$@"
              ''}";
              files = "^justfile$";
              pass_filenames = true;
            };

            # Check for large files
            check-added-large-files = {
              enable = true;
              entry = "${pkgs.writeShellScript "check-large-files" ''
                for file in "$@"; do
                  size=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null)
                  if [ "$size" -gt 512000 ]; then
                    echo "Error: $file is larger than 500KB (size: $size bytes)"
                    exit 1
                  fi
                done
              ''}";
              pass_filenames = true;
            };

            # JSON/YAML/TOML validation
            check-json = {
              enable = true;
              entry = "${pkgs.jq}/bin/jq empty";
              types = [ "json" ];
              pass_filenames = true;
            };

            check-yaml = {
              enable = false; # No YAML files in the project
              entry = "${pkgs.writeShellScript "check-yaml" ''
                if [ $# -eq 0 ]; then
                  exit 0
                fi
                for file in "$@"; do
                  ${pkgs.yq}/bin/yq e '.' "$file" > /dev/null || exit 1
                done
              ''}";
              types = [ "yaml" ];
              pass_filenames = true;
            };

            check-toml = {
              enable = true;
              entry = "${pkgs.taplo}/bin/taplo check";
              types = [ "toml" ];
              pass_filenames = true;
            };

            # Check merge conflicts
            check-merge-conflict = {
              enable = true;
              entry = "${pkgs.writeShellScript "check-merge-conflict" ''
                ${pkgs.ripgrep}/bin/rg -n '^<<<<<<< |^======= |^>>>>>>> ' "$@" && exit 1 || exit 0
              ''}";
              pass_filenames = true;
            };

            # End of file fixer
            end-of-file-fixer = {
              enable = true;
              entry = "${pkgs.writeShellScript "end-of-file-fixer" ''
                for file in "$@"; do
                  if [ -f "$file" ] && [ -s "$file" ]; then
                    tail -c1 "$file" | read -r _ || echo >> "$file"
                  fi
                done
              ''}";
              excludes = [
                ".*\\.svg$"
                ".*\\.png$"
                ".*\\.jpg$"
              ];
              pass_filenames = true;
            };

            # Trailing whitespace
            trailing-whitespace = {
              enable = true;
              entry = "${pkgs.writeShellScript "trailing-whitespace" ''
                ${pkgs.gnused}/bin/sed -i 's/[[:space:]]*$//' "$@"
              ''}";
              excludes = [ ".*\\.md$" ];
              pass_filenames = true;
            };
          };
        };
      in
      {
        # Make pre-commit-check available for `nix flake check`
        checks = {
          inherit pre-commit-check;
        };

        packages = {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "claudius";
            version = "0.1.0";
            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };

            # Note: On Darwin, Security and SystemConfiguration frameworks are now
            # provided automatically by the Darwin stdenv (nixpkgs 25.05+).
            # See: https://discourse.nixos.org/t/the-darwin-sdks-have-been-updated/55295
            buildInputs = with pkgs; [
              pkg-config
              openssl
            ];

            nativeBuildInputs = [ pkgs.pkg-config ];

            doCheck = true;

            # Enable test mocking for 1Password CLI in Nix build environment
            preCheck = ''
              export CLAUDIUS_TEST_MOCK_OP=1
            '';

            # Clippy configuration is in clippy.toml and .cargo/config.toml
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs =
            with pkgs;
            [
              rustToolchain
              rust-analyzer
              cargo-watch
              cargo-nextest
              cargo-outdated
              cargo-audit
              cargo-llvm-cov
              cargo-machete
              cargo-deny
              pkg-config
              openssl
              just
              tokei
              ripgrep
              # Additional tools for hooks
              taplo
              typos
              markdownlint-cli
              shellcheck
              nixpkgs-fmt
              # Nix linting and formatting tools (per linter-policy-nix.md)
              nixfmt-rfc-style
              statix
              deadnix
              nil
              # Act for local GitHub Actions testing (per ci-policy-github-actions.md)
              act
            ]
            ++ pre-commit-check.enabledPackages;

          # Combined shell hook
          shellHook = ''
            ${pre-commit-check.shellHook}

            echo "🚀 Claudius development environment"
            echo "Rust version: $(rustc --version)"
            echo "Just version: $(just --version)"
            echo ""
            echo "📋 Pre-commit hooks are installed and active"
            echo "Available commands: just --list"
            echo ""
            echo "🔍 Run 'nix flake check' to run all checks"
            echo "🪝 Run 'pre-commit run --all-files' to run all hooks manually"
          '';
        };

        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };
      }
    );
}
