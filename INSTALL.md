# `superfuck` install guide

This project has two command surfaces:

- `superfuck`: the binary name
- `fuck`: the default shell UX installed by `superfuck init`

Use this guide if you want the `thefuck`-style flow where you type `fuck` after a failed command.
Provider names are your config section names, and they support prefix matching.

## 1. Install the binary

```bash
cargo install --path .
```

## 2. Initialize configuration

The configuration file is located at `~/.config/superfuck/config.toml`.

The following command creates the configuration file with default values.

```bash
superfuck config init
```

The generated template includes `chatgpt`, `claude`, `gemini`, and `local` providers. Edit the one you want to use first, then export the matching API key env var if needed.

Some configuration values can be overridden with environment variables:

```bash
export SUPERFUCK_MAX_ALTERNATIVES=3
export SUPERFUCK_LANGUAGE="Japanese"
export SUPERFUCK_PROVIDER="qwen"
export SUPERFUCK_SYSTEM_PROMPT='Return strict JSON only.'
```

## 3. Enable `fuck` on your shell

### zsh

Add below line to your `~/.zshrc`:

```zsh
eval "$(superfuck init zsh)"
```

If you think the default alias name `fuck` is not good, you can change it with `--as` option.

```zsh
eval "$(superfuck init zsh --as wtf)"
```

NOTE: Please reload your shell after adding the line above.

### bash

Add below line to your `~/.bashrc`:

```bash
eval "$(superfuck init bash)"
```

If you think the default alias name `fuck` is not good, you can change it with `--as` option.

```bash
eval "$(superfuck init bash --as wtf)"
```

NOTE: Please reload your shell after adding the line above.

## 4. Use it

You can `fuck` immediately after a failed command.

```bash
gti status
fuck
```

And you can switch the provider with `fuck <provider_name>` if you configured multiple providers correctly.

```bash
fuck openai
fuck claude
fuck gemini
fuck local
```

## 5. Verify the configuration

If `fuck` is not working as expected, check the current configuration with:

```bash
superfuck doctor
```

To check a specific provider:

```bash
superfuck --provider <provider_name> doctor
```

## 6. Local LLM Example

If you want to use a local LLM, it must expose an OpenAI-compatible API.
For example, Qwen2.5-Coder-14B-Instruct-4bit on MLX:

```bash
brew install mlx-lm
mlx-lm --model Qwen2.5-Coder-14B-Instruct-4bit --server
```

Then, you can configure `superfuck` to use the local LLM.

```toml
[providers.mlx-qwen25]
backend = "openai-compatible"
base_url = "http://127.0.0.1:8080/v1"
model = "mlx-community/Qwen2.5-Coder-14B-Instruct-4bit"
```

Then run `fuck mlx-qwen25` to use the local LLM.

## 7. Uninstall

To uninstall `superfuck`, remove the installed binary and delete the `eval "$(superfuck init ...)"` line from your shell configuration file.

```bash
# If you installed it with cargo
cargo uninstall superfuck
```

Then open your shell config file, such as `~/.zshrc` or `~/.bashrc`, and remove the line that evaluates `superfuck init`.
