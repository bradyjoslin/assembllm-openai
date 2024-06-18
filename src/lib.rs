use extism_pdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, str::from_utf8};

#[derive(Serialize)]
struct WrappedTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: ToolFunction,
}

#[derive(Serialize)]
struct ToolFunction {
    name: String,
    description: String,
    parameters: FunctionParameters,
}

#[derive(Serialize)]
struct FunctionParameters {
    #[serde(rename = "type")]
    param_type: String,
    properties: HashMap<String, serde_json::Value>,
    required: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ToolResult {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    function: ToolFunctionResult,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ToolFunctionResult {
    name: String,
    arguments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    content: Option<String>,
    role: String,
    tool_calls: Option<Vec<ToolResult>>,
}

#[derive(Serialize, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Serialize, Deserialize)]
struct ChatResult {
    choices: Vec<ChatChoice>,
}

#[derive(Serialize, Deserialize, FromBytes)]
#[encoding(Json)]
pub struct InputSchema {
    #[serde(rename = "type")]
    pub data_type: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub required: Vec<String>,
}

#[derive(Serialize, Deserialize, FromBytes)]
#[encoding(Json)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, FromBytes)]
#[encoding(Json)]
pub struct Tool {
    pub name: Option<String>,
    pub description: Option<String>,
    pub input_schema: InputSchema,
    #[serde(default = "default_type")]
    pub r#type: String,
}

fn default_type() -> String {
    "function".to_string()
}

#[derive(Serialize, Deserialize, FromBytes)]
#[encoding(Json)]
pub struct CompletionToolInput {
    pub tools: Vec<Tool>,
    pub messages: Vec<Message>,
}

#[derive(Debug)]
struct OpenAIConfig {
    api_key: String,
    model: Model,
    temperature: f32,
    role: String,
}

#[derive(Clone, Debug, Serialize)]
struct Model {
    name: &'static str,
    aliases: [&'static str; 1],
}

static MODELS: [Model; 8] = [
    Model {
        name: "gpt-4o",
        aliases: ["4o"],
    },
    Model {
        name: "gpt-4",
        aliases: ["4"],
    },
    Model {
        name: "gpt-4-1106-preview",
        aliases: ["128k"],
    },
    Model {
        name: "gpt-4-32k",
        aliases: ["32k"],
    },
    Model {
        name: "gpt-3.5-turbo",
        aliases: ["35t"],
    },
    Model {
        name: "gpt-3.5-turbo-1106",
        aliases: ["35t-1106"],
    },
    Model {
        name: "gpt-3.5-turbo-16k",
        aliases: ["35t16k"],
    },
    Model {
        name: "gpt-3.5",
        aliases: ["35"],
    },
];

fn get_completion(
    api_key: String,
    model: &Model,
    prompt: String,
    temperature: f32,
    role: String,
    tools: Option<Vec<Tool>>,
) -> Result<ChatResult, anyhow::Error> {
    let req = HttpRequest::new("https://api.openai.com/v1/chat/completions")
        .with_header("Authorization", format!("Bearer {}", api_key))
        .with_header("Content-Type", "application/json")
        .with_method("POST");

    let mut wrapped_tools: Vec<WrappedTool> = Vec::new();
    match tools {
        Some(tools) => {
            info!("Tools found");
            wrapped_tools = tools
                .into_iter()
                .map(|tool| WrappedTool {
                    tool_type: "function".to_string(),
                    function: ToolFunction {
                        name: tool.name.unwrap_or_default(),
                        description: tool.description.unwrap_or_default(),
                        parameters: FunctionParameters {
                            param_type: tool.input_schema.data_type,
                            properties: tool.input_schema.properties,
                            required: tool.input_schema.required,
                        },
                    },
                })
                .collect();
        }
        None => {
            info!("No tools found");
        }
    }

    // We could make our own structs for the body
    // this is a quick way to make some unstructured JSON
    let mut req_body = json!({
        "model": model.name,
        "temperature": temperature,
        "messages": [
            {
                "role": "system",
                "content": role,
            },
            {
                "role": "user",
                "content": prompt
            }
        ]
    });

    if !wrapped_tools.is_empty() {
        req_body["tools"] = json!(wrapped_tools);
        req_body["tool_choice"] = "required".into();
    }

    let res = http::request::<String>(&req, Some(req_body.to_string()))?;
    match res.status_code() {
        200 => {
            info!("Request successful");
        }
        _ => {
            let response_body = res.body();
            let body = from_utf8(&response_body)?;
            return Err(anyhow::anyhow!(
                "error calling API\nStatus Code: {}\n Response: {}",
                res.status_code(),
                body
            ));
        }
    }
    let response_body = res.body();
    let body = from_utf8(&response_body)?;

    let chat_result: ChatResult = serde_json::from_str(body)?;
    Ok(chat_result)
}

fn get_config_values(
    cfg_get: impl Fn(&str) -> Result<Option<String>, anyhow::Error>,
) -> FnResult<OpenAIConfig> {
    let api_key = cfg_get("api_key")?;
    let model_input = cfg_get("model")?;
    let temperature_input = cfg_get("temperature")?;
    let role_input = cfg_get("role")?;

    match api_key {
        Some(_) => {
            info!("API key found");
        }
        None => {
            error!("API key not found");
            return Err(WithReturnCode::new(anyhow::anyhow!("API key not found"), 1));
        }
    }

    let model = match model_input {
        Some(model) => {
            let found_model = MODELS.iter().find(|m| {
                m.name.to_lowercase() == model.to_lowercase()
                    || m.aliases
                        .iter()
                        .any(|&alias| alias.to_lowercase() == model.to_lowercase())
            });
            match found_model {
                Some(m) => {
                    info!("Model found: {}", m.name);
                    m
                }
                None => {
                    error!("Model not found");
                    return Err(WithReturnCode::new(anyhow::anyhow!("Model not found"), 1));
                }
            }
        }
        _ => {
            info!("Model not specified, using default");
            MODELS.first().unwrap()
        }
    };

    let temperature = match temperature_input {
        Some(temperature) => {
            let t = temperature.parse::<f32>();
            match t {
                Ok(t) => {
                    if t < 0.0 || t > 1.0 {
                        error!("Temperature must be between 0.0 and 1.0");
                        return Err(WithReturnCode::new(
                            anyhow::anyhow!("Temperature must be between 0.0 and 1.0"),
                            1,
                        ));
                    }
                    info!("Temperature: {}", t);
                    t
                }
                Err(_) => {
                    error!("Temperature must be a float");
                    return Err(WithReturnCode::new(
                        anyhow::anyhow!("Temperature must be a float"),
                        1,
                    ));
                }
            }
        }
        None => {
            info!("Temperature not specified, using default");
            0.7
        }
    };

    let role = role_input.unwrap_or("".to_string());
    if role != "" {
        info!("Role: {}", role);
    } else {
        info!("Role not specified");
    }

    Ok(OpenAIConfig {
        api_key: api_key.unwrap(),
        model: model.clone(),
        temperature,
        role,
    })
}

#[plugin_fn]
pub fn completion(input: String) -> FnResult<String> {
    let cfg = get_config_values(|key| config::get(key))?;

    let res = get_completion(
        cfg.api_key,
        &cfg.model,
        input,
        cfg.temperature,
        cfg.role,
        None,
    )?;

    let output = res.choices[0].message.content.clone();

    match output {
        Some(output) => Ok(output),
        None => Err(WithReturnCode::new(
            anyhow::anyhow!("No completion returned"),
            1,
        )),
    }
}

#[plugin_fn]
pub fn completionWithTools(input: CompletionToolInput) -> FnResult<String> {
    let cfg = get_config_values(|key| config::get(key))?;

    let prompt = input.messages[0].content.clone();
    let res = get_completion(
        cfg.api_key,
        &cfg.model,
        prompt,
        cfg.temperature,
        cfg.role,
        Some(input.tools),
    )?;

    let tool_calls = res.choices[0]
        .message
        .tool_calls
        .as_ref()
        .ok_or(anyhow::anyhow!("No tool calls found"))?;

    let formatted_tool_calls: Vec<Value> = tool_calls
        .iter()
        .map(|tool_call| {
            let mut tool_call_json = json!({
                "name": tool_call.function.name,
            });

            if let Some(arguments_str) = &tool_call.function.arguments {
                let arguments_json: Value = serde_json::from_str(arguments_str).unwrap();
                tool_call_json["input"] = arguments_json;
            }

            tool_call_json
        })
        .collect();

    let json_output = serde_json::to_string_pretty(&formatted_tool_calls)?;
    Ok(json_output)
}

#[plugin_fn]
pub fn models() -> FnResult<String> {
    let models_json = serde_json::to_string(&MODELS)?;
    info!("Returning models");
    Ok(models_json)
}
