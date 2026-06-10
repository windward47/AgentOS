/**
 * Test: Make a real LLM call through pi-ai's streamSimple.
 * Uses MiMo V2.5 Pro on Xiaomi (sensenova) provider.
 */
import { streamSimple, type Context } from "@oh-my-pi/pi-ai";
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

console.log("Model:", model.provider, model.id, model.api);

async function main() {
    console.log("\nCalling Xiaomi MiMo V2.5 Pro...\n");

    const context: Context = {
        systemPrompt: ["You are a helpful assistant."],
        messages: [
            { role: "user", content: "Say hello in exactly 5 words." },
        ],
    };

    try {
        const stream = streamSimple(
            model as any,
            context,
            {
                apiKey: "tp-cyfh35ww453t75qy9rqdmvmcldq0k0egefj0lnbuz5eax6v3",
                maxTokens: 1024,
            },
        );

        let fullText = "";
        for await (const event of stream) {
            if (event.type === "text_delta") {
                fullText += event.delta;
                process.stdout.write(event.delta);
            } else if (event.type === "thinking_delta") {
                // skip
            } else if (event.type === "stop") {
                console.log("\n\nStop reason:", event.stopReason);
            } else if (event.type === "error") {
                console.error("\nError:", event.error);
            }
        }

        console.log("\nFull response:", JSON.stringify(fullText));
        console.log("\n✅ Test complete!");
    } catch (err) {
        console.error("Fatal error:", err);
    }
}

main();
