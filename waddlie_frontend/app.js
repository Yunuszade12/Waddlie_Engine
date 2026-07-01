

let workspace;
let selectedEntityId = null;
const bevyCanvas = document.getElementById('bevy-canvas');

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

        if (bevyCanvas) {
            // Stops the browser inspect menu on right-click
            bevyCanvas.addEventListener('contextmenu', (e) => {
                e.preventDefault();
            });

            // Forces keyboard focus into Bevy when clicked
            bevyCanvas.addEventListener('mousedown', () => {
                bevyCanvas.focus();
            });

            // 🚀 NEW: Disable default browser shortcuts (like Spacebar scrolling) 
            // ONLY when the user is actively focused on the Bevy game view canvas.
            bevyCanvas.addEventListener('keydown', (e) => {
                const keysToBlock = [
                    ' ',            // Spacebar (prevents page scrolling down)
                    'ArrowUp',      // Up Arrow
                    'ArrowDown',    // Down Arrow
                    'ArrowLeft',    // Left Arrow
                    'ArrowRight',   // Right Arrow
                    'Tab'           // Tab (optional: stops focus from leaving canvas)
                ];

                if (keysToBlock.includes(e.key)) {
                    e.preventDefault();
                }
            });
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

// Deserializes scene.json and populates the entity selection buttons list
async function loadSceneAndPopulateUI() {
    try {

        const response = await fetch('assets/scene.json'); // ✅ Pulls live from Vite server
        if (!response.ok) throw new Error("Failed to fetch scene file");

        const sceneData = await response.json();
        // Double-check this matches your local relative route file path

        console.log("Deserialized Scene Object Array:", sceneData);

        // 1. Extract only the entries marked as standard "Entity" types
        const entities = sceneData.filter(item => item.type === "Entity");

        const container = document.getElementById('entities-list-container');
        container.innerHTML = ""; // Clean up placeholder template code

        // 2. Build out an array sequence of functional button items
        entities.forEach(entity => {
            const button = document.createElement('button');
            button.className = 'entity-btn';
            button.innerText = `ID ${entity.id}: ${entity.name}`;

            // 3. Bind interactive tap listener event profiles
            button.addEventListener('click', () => {
                // Toggle focus selection stylings visually
                document.querySelectorAll('.entity-btn').forEach(btn => btn.classList.remove('active'));
                button.classList.add('active');

                selectedEntityId = entity.id;
                console.log(`Focused Scope Target Entity ID: ${selectedEntityId}`);

                // Automatically trigger updating the right panel with its inner components!
                renderComponentsForEntity(entity);
            });

            container.appendChild(button);
        });

    } catch (err) {
        console.error("Error processing scene file integration UI layout:", err);
    }
}

// Render the components inside the right-hand panel column section
// Render components as clickable buttons and reveal their data specs when clicked
function renderComponentsForEntity(entity) {
    const compContainer = document.getElementById('components-list-container');
    compContainer.innerHTML = ""; // Wipe past entity properties

    // 1. Gather all components into a uniform array list
    const allComponents = [];

    // Synthesize a virtual Transform component using properties directly on the entity
    allComponents.push({
        type: "Transform",
        position_x: entity.position_x,
        position_y: entity.position_y,
        position_z: entity.position_z,
        rotation_x: entity.rotation_x,
        rotation_y: entity.rotation_y,
        rotation_z: entity.rotation_z,
        scale_x: entity.scale_x,
        scale_y: entity.scale_y,
        scale_z: entity.scale_z
    });

    // Append remaining custom components if they exist
    if (entity.components && entity.components.length > 0) {
        entity.components.forEach(comp => allComponents.push(comp));
    }

    // 2. Loop through and build button actions for every component
    allComponents.forEach((comp, index) => {
        const compWrapper = document.createElement('div');
        compWrapper.style.marginBottom = "8px";

        // Create the component button element
        const btn = document.createElement('button');
        btn.className = 'component-btn';
        btn.innerText = `${comp.type}`;

        // Create a hidden panel for its properties (specs)
        const specsPanel = document.createElement('div');
        specsPanel.className = 'specs-panel';
        specsPanel.style.display = 'none'; // Hidden by default

        // Build out lines for all properties inside this component object
        Object.keys(comp).forEach(key => {
            // Skip showing the "type" property as a spec since it's already the header title
            if (key === 'type') return;

            const specLine = document.createElement('div');
            specLine.className = 'spec-line';

            const keySpan = document.createElement('span');
            keySpan.className = 'spec-key';
            keySpan.innerText = `${key}:`;

            const valSpan = document.createElement('span');
            valSpan.className = 'spec-val';

            // Handle arrays/objects (like color_rgb arrays) vs basic strings or floats cleanly
            let val = comp[key];
            if (typeof val === 'object' && val !== null) {
                valSpan.innerText = JSON.stringify(val);
            } else {
                valSpan.innerText = val !== undefined ? val : "null";
            }

            specLine.appendChild(keySpan);
            specLine.appendChild(valSpan);
            specsPanel.appendChild(specLine);
        });

        // 3. Attach a toggle switch click handler to view or hide properties
        btn.addEventListener('click', () => {
            const isCurrentlyVisible = specsPanel.style.display === 'block';

            // Close any currently opened spec panels in the column
            document.querySelectorAll('.specs-panel').forEach(p => p.style.display = 'none');
            document.querySelectorAll('.component-btn').forEach(b => b.classList.remove('active'));

            // Toggle active state
            if (!isCurrentlyVisible) {
                specsPanel.style.display = 'block';
                btn.classList.add('active');
            }
        });

        compWrapper.appendChild(btn);
        compWrapper.appendChild(specsPanel);
        compContainer.appendChild(compWrapper);
    });
}

// Bind UI translation triggers
document.getElementById('btn-en').addEventListener('click', () => loadEditorConfig('en'));

// Initialize with default setting profile
// Replace the block at the very bottom of app.js with this:
function initializeApp() {
    const blocklyDiv = document.getElementById('blockly-div');
    const bevyCanvas = document.getElementById('bevy-canvas');

    // 🛠️ Double check that the HTML elements are fully loaded in the document
    if (!blocklyDiv) {
        console.warn("Waiting for DOM elements to fully attach... Retrying in 50ms");
        setTimeout(initializeApp, 50);
        return;
    }

    console.log("DOM fully ready! Initializing editor interfaces...");
    loadEditorConfig('en');
    loadSceneAndPopulateUI();

    // Stops the browser inspect menu on right-click
    if (bevyCanvas) {
        bevyCanvas.addEventListener('contextmenu', (e) => {
            e.preventDefault();
        });

        // Forces keyboard focus into Bevy when clicked
        bevyCanvas.addEventListener('mousedown', () => {
            bevyCanvas.focus();
        });
    }
}

// Start checking as soon as possible
if (document.readyState === 'loading') {
    window.addEventListener('DOMContentLoaded', initializeApp);
} else {
    initializeApp();
}