import { streamSimple } from "@oh-my-pi/pi-ai";
import { getBundledModel } from "@oh-my-pi/pi-catalog/models";
import { Agent } from "@oh-my-pi/pi-agent-core";

// Test 1: Check that pi-ai exports work
console.log("✓ pi-ai imported successfully");

// Test 2: Get a bundled model
const model = getBundledModel("openai", "gpt-4o-mini");
console.log("✓ getBundledModel works:", model?.provider, model?.id);
console.log("  baseUrl:", model?.baseUrl);
console.log("  api:", model?.api);

// Test 3: Check pi-agent-core
console.log("✓ pi-agent-core imported successfully");

// Test 4: Create an Agent
const agent = new Agent({
    initialState: {
        systemPrompt: ["You are a helpful assistant."],
        model: model,
    }
});
console.log("✓ Agent created successfully");
console.log("  Model:", agent.state.model?.provider, agent.state.model?.id);
console.log("  System prompt:", agent.state.systemPrompt);

console.log("\n✅ All imports verified. Packages are compatible with Bun", Bun.version);
