OPENAI_API_KEY := $(shell echo $$OPENAI_API_KEY)

build:
	cargo build --release --target wasm32-unknown-unknown

test:
	extism call ./target/wasm32-unknown-unknown/release/assembllm_openai.wasm models --log-level=info
	@extism call ./target/wasm32-unknown-unknown/release/assembllm_openai.wasm completion \
		--set-config='{"api_key": "$(OPENAI_API_KEY)"}' \
		--input="Explain extism in the context of wasm succinctly." \
		--allow-host=api.openai.com --log-level=info
	@extism call ./target/wasm32-unknown-unknown/release/assembllm_openai.wasm completionWithTools \
		--input '{"tools": [{"name": "get_weather","description": "Get the current weather in a given location","input_schema": {"type": "object","properties": {"location": {"type": "string","description": "The city and state, e.g. San Francisco, CA"},"unit": {"type": "string","enum": ["celsius"],"description": "The unit of temperature, always celsius"}},"required": ["location", "unit"]}}],"messages": [{"role": "user","content": "What is the weather like in San Francisco?"}]}' \
		--wasi --allow-host "api.openai.com" --log-level info \
		--set-config='{"api_key": "$(OPENAI_API_KEY)"}'
	