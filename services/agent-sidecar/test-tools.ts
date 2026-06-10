/**
 * Test: Agent class with tool calling.
 */
import { Agent, type AgentTool } from "@oh-my-pi/pi-agent-core";
import { buildModel } from "@oh-my-pi/pi-catalog/build";

const model = buildModel({
    provider: "sensenova",
    id: "mimo-v2.5-pro",
    name: "MiMo V2.5 Pro",
    api: "openai-completions",
    baseUrl: "https://token-plan-cn.xiaomimimo.com/v1",
    reasoning: false,
    input: ["text"],
    cost: {
        input: 0,
        output: 0,
        cacheRead: 0,
        cacheWrite: 0,
    },
    contextWindow: 1048576,
    maxTokens: 65536,
});

console.log("Model:", model.provider, model.id);

async function main() {
    const agent = new Agent({
        initialState: {
            systemPrompt: ["You are a helpful assistant."],
            model: model as any,
            tools: [
                {
                    name: "get_weather",
                    label: "Get Weather",
                    description: "Get the current weather for a city",
                    parameters: {
                        type: "object",
                        properties: {
                            city: { type: "string", description: "City name" },
                        },
                        required: ["city"],
                    },
                    execute: async (toolCallId, params, signal, onUpdate, context) => {
                        console.log(`\n[Tool executed] get_weather(${params.city})`);
                        return {
                            content: [{ type: "text", text: `The weather in ${params.city} is sunny, 22°C.` }],
                        };
                    },
                } as AgentTool,
            ],
        },
        getApiKey: () => "tp-cyfh35ww453t75qy9rqdmvmcldq0k0egefj0lnbuz5eax6v3",
    });

    agent.subscribe((event) => {
        if (event.type === "message_update" && event.assistantMessageEvent.type === "text_delta") {
            process.stdout.write(event.assistantMessageEvent.delta);
        } else if (event.type === "tool_execution_start") {
            console.log(`\n[Tool start] ${event.toolName}`);
        } else if (event.type === "tool_execution_end") {
            console.log(`\n[Tool end] ${event.toolName}, result:`, JSON.stringify(event.result).slice(0, 200));
        }
    });

    console.log("Calling Agent.prompt('What is the weather in Beijing?')...\n");
    const result = await agent.prompt("What is the weather in Beijing?");
    console.log("\n\n✅ Agent tool-calling works!");
}

main().catch(console.error);
