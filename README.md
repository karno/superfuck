# 𝒔𝒖𝒑𝒆𝒓𝒇𝒖𝒄𝒌: LLM-powered `thefuck`

`superfuck` is a [`thefuck`](https://github.com/nvbn/thefuck)-like CLI that uses an LLM to suggest corrected shell commands.

The binary name is `superfuck`. The default interactive shell UX is `fuck`, but it can be configured while installation.

Currently, we supports the following LLM providers via each API:

- OpenAI ChatGPT
- Anthropic Claude
- Google Gemini
- OpenAI-compatible providers (including local ones like Ollama, vLLM, LM Studio, etc.)

You may need to valid Pay-as-you-go API key for each provider, or set up your own LLM server.

## Installation

TL;DR: `cargo install --path .`

Detailed instructions are in [INSTALL.md](./INSTALL.md).

## Commands

- just `fuck` after failed command

## Development

```bash
cargo test
```

## License

Licensed under either of:

- MIT license ([LICENSE-MIT](./LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))

at your option.
