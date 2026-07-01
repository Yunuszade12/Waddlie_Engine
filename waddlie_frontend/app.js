import * as Blockly from 'blockly';

let workspace;

async function loadEditorConfig(lang) {
    try {
        const response = await fetch(`/locales/blocks.json-${lang}.json`);
        if (!response.ok) throw new Error("Could not fetch data configuration");

        const config = await response.json();

        // 1. Wipe out any previous custom blocks to prevent overwrite crashes
        config.blocks.forEach(blockData => {
            if (Blockly.Blocks[blockData.type]) {
                delete Blockly.Blocks[blockData.type];
            }
        });

        // 2. Register all blocks cleanly out of the JSON array definition
        Blockly.defineBlocksWithJsonArray(config.blocks);

        // 3. Keep track of the current layout block layout state if changing language mid-session
        let currentWorkspaceState = null;
        if (workspace) {
            currentWorkspaceState = Blockly.serialization.workspaces.save(workspace);
            workspace.dispose();
        }

        // 4. Inject workspace directly passing the JSON toolbox setup
        workspace = Blockly.inject('blockly-div', {
            toolbox: config.toolbox, // Pure JSON format toolbox
            scrollbars: true,
            trashcan: true,
            media: 'https://unpkg.com/blockly/media/'
        });

        // 5. Restore user's workspace blocks state if it exists
        if (currentWorkspaceState) {
            Blockly.serialization.workspaces.load(currentWorkspaceState, workspace);
        }

    } catch (err) {
        console.error("Failed to load JSON workspace config:", err);
    }
}


//Lets desirleize scene.json so we can attach componnets to entitites:
async function loadSceneData() {
    try {
        const response = await fetch('/path/to/scene.json');
        if (!response.ok) throw new Error("Failed to fetch scene file");

        // .json() automatically turns the raw text string into a JS object/array
        const sceneData = await response.json();

        console.log("Deserialized Scene Object:", sceneData);
        return sceneData;
    } catch (err) {
        console.error("Error parsing scene JSON:", err);
    }
}



// Bind UI translation triggers
document.getElementById('btn-en').addEventListener('click', () => loadEditorConfig('en'));

// Initialize with default setting profile
window.addEventListener('DOMContentLoaded', () => {
    loadEditorConfig('en');
});