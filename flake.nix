{
  description = "Dev shell with Node.js 22, PNPM, and TypeScript Language Server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/master";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
        nodePackages = pkgs.nodePackages;
      in {
        devShells.default = pkgs.mkShell {
          name = "dev-shell";

          buildInputs = [
            pkgs.openssl
            pkgs.pkg-config
            pkgs.nodejs_22
            pkgs.pnpm
            pkgs.git
            pkgs.lazyjj
            pkgs.jujutsu
	    # typescript stuff
            nodePackages.typescript-language-server
            nodePackages.typescript
          ];

          shellHook = ''
            echo "✅ Frontend dev shell with TypeScript LSP"
            echo "➡️  Node: $(node -v)"
            echo "➡️  PNPM: $(pnpm -v)"
            echo "➡️  TSServer: $(typescript-language-server --version)"
          '';
        };
      });
}

