# OpenRouter Free Gateway (`openrouter-free-gateway`)

A high-performance, asynchronous Rust proxy service that dynamically discovers and exposes strictly **100% free models** (zero cost for prompt and completion tokens) from [OpenRouter](https://openrouter.ai), filtering them on the fly and serving as an OpenAI-compatible endpoint for coding agents and developer tooling (e.g., Oh My Pi / OMP, Zed, Claude Code).

---

## ✨ Features

- **Dynamic Zero-Cost Filtering**: Periodically queries `https://openrouter.ai/api/v1/models` and admits only models where:
  - Input price is zero (`pricing.prompt == "0"`).
  - Output price is zero (`pricing.completion == "0"`).
  - Model identifier carries `:free` or matches `openrouter/free`.
  - Tool calling is natively supported (`supported_parameters` contains `"tools"`).
- **Type-Driven Architecture**: Fully enum-based classification pipeline (`TokenPrice`, `PricingTier`, `ModelTag`, `ToolCapability`, `ModelEligibility`) with zero ambiguous conditional branches.
- **Strict Guardrail Proxy**: `POST /v1/chat/completions` validates requested models against the live free whitelist and instantly rejects non-free models with `HTTP 403 Forbidden` (`permission_denied`), preventing unexpected charges.
- **Transparent Streaming**: Supports server-sent events (`text/event-stream`) for real-time token streaming with low overhead.
- **In-Memory Caching**: Caches discovered model lists with configurable TTL (default: 5 minutes) to avoid rate-limiting upstream endpoints.
- **Declarative Nix Flake**: Packaged with a precompiled binary in `dist/` for instant Nix derivation builds (0.05s) alongside fallback from-source compilation.

---

## 🚀 Quick Start

### Build & Run Locally

```bash
# Build with Rust nightly (Cranelift codegen enabled)
cargo build --release -j 2

# Run gateway on port 4005
./target/release/openrouter-free-gateway --port 4005
```

### CLI Arguments & Environment Variables

| Flag | Env Variable | Default | Description |
| :--- | :--- | :--- | :--- |
| `--host` | `GATEWAY_HOST` | `127.0.0.1` | Bind interface |
| `-p`, `--port` | `GATEWAY_PORT` | `4005` | Bind port |
| `--openrouter-url` | `OPENROUTER_BASE_URL` | `https://openrouter.ai/api/v1` | Upstream OpenRouter endpoint |
| `--cache-ttl-secs` | `GATEWAY_CACHE_TTL_SECS` | `300` | In-memory catalog cache TTL |
| `--api-key` | `OPENROUTER_API_KEY` | None | Default fallback Bearer token |

---

## 🔌 API Endpoints

- **`GET /health`**: Healthcheck probe (`{"status":"ok","service":"openrouter-free-gateway"}`).
- **`GET /v1/models`** (or `/models`): OpenAI-compatible model list filtered to verified free models.
- **`POST /v1/chat/completions`**: Validated streaming completions forwarder.

---

## 📦 NixOS Declarative Integration

Add to your `flake.nix` inputs:

```nix
openrouter-free-gateway = {
  url = "git+ssh://git@github.com/RikaKit2/openrouter-free-gateway.git";
  inputs.nixpkgs.follows = "nixpkgs-unstable";
};
```

And in your systemd user service module:

```nix
{ inputs, pkgs, ... }:
let
  pkg = inputs.openrouter-free-gateway.packages.${pkgs.stdenv.hostPlatform.system}.default;
in
{
  environment.systemPackages = [ pkg ];

  systemd.user.services."openrouter-free-gateway" = {
    description = "OpenRouter Free Gateway Service";
    after = [ "network.target" ];
    wantedBy = [ "default.target" ];

    serviceConfig = {
      ExecStart = "${pkg}/bin/openrouter-free-gateway --port 4005";
      Restart = "always";
      RestartSec = 3;
      MemoryMax = "256M";
      CPUQuota = "100%";
    };
  };
}
```

---

## 🛠️ Toolchain Standards

Built strictly to workstation specifications:
- **Rust Toolchain**: `nightly` with `rustc-codegen-cranelift`.
- **Linker**: `clang` with `-C link-arg=--ld-path=wild` (Wild linker).
- **Code Quality**: Checked by `prek` (`fmt`, `clippy -D warnings`, `test`, `audit`).

---

## 📄 License

MIT License.
