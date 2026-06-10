/**
 * Test: Verify Agent class from @oh-my-pi/pi-agent-core works
 * with a custom model and can stream responses.
 */
import { Agent } from "@oh-my-pi/pi-agent-core";
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
        },
        getApiKey: () => "tp-cyfh35ww453t75qy9rqdmvmcldq0k0egefj0lnbuz5eax6v3",
    });

    // Subscribe to events
    agent.subscribe((event) => {
        if (event.type === "message_update" && event.assistantMessageEvent.type === "text_delta") {
            process.stdout.write(event.assistantMessageEvent.delta);
        }
    });

    console.log("Calling Agent.prompt('Say hello in 5 words')...\n");
    const result = await agent.prompt("Say hello in exactly 5 words.");
    console.log("\n\nAgent result:", JSON.stringify(result?.content?.slice(0, 100)));
    console.log("\n✅ Agent class works!");
}

main().catch(console.error);
