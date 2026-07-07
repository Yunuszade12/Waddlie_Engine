let workspace;
let selectedEntityId = null;
const bevyCanvas = document.getElementById('bevy-canvas');
let currentFolderView = "root"; // Modes: "root" or "models"
const importedModelFileList = [];

const uploadedAssetsMemoryCache = new Map();


import * as wasmEngine from './wasm/waddlie_core.js';

/**
 * Update transform parameters (Nudge Translation, Scale, or Rotation)
 * @param {number} entityId - The unique JSON ID of the entity
 * @param {string} transformType - 'nudge_x', 'nudge_y', 'nudge_z', 'scale_x', 'scale_y', 'scale_z', 'rotation_x', 'rotation_y', 'rotation_z'
 * @param {number} numericValue - The new target coordinate/scale factor/angle
 */

export function updateEntityTransform(entityId, transformType, numericValue) {
    // We pass an empty string for the unused text argument slot
    wasmEngine.order_benvy(entityId, transformType, "", numericValue);
}

/**
 * Dynamic GLTF Hot-Swapping
 * @param {number} entityId - The unique JSON ID of the entity
 * @param {string} glbFileName - The name of the file registered in Bevy's VFS (e.g., 'soldier.glb')
 */
export function hotSwapGltfModel(entityId, glbFileName) {
    if (!entityId) {
        console.warn("Cannot hot-swap: No entity ID provided.");
        return;
    }
    // Pass 0.0 for the unused numeric value argument slot[cite: 10]
    wasmEngine.order_benvy(entityId, "model_path", glbFileName, 0.0);
}






// Bevy will trigger this when single-clicking a scene target inside the 3D viewport
window.notifyJsEntitySelected = function (jsonId) {
    console.log(`📡 Bevy selected Entity ID: ${jsonId}`);
    const buttons = document.querySelectorAll('.entity-btn');
    buttons.forEach(button => {
        if (button.innerText.startsWith(`ID ${jsonId}:`)) {
            button.click(); // Synchronize UI state
        }
    });
};

// Bevy automatically calls this when bones are clicked in selection mode
window.updateJsSelectedBonesList = function (bonesJson) {
    const bonesArray = JSON.parse(bonesJson);
    const container = document.getElementById('selected-bones-list');
    container.innerHTML = "";

    if (bonesArray.length === 0) {
        container.innerHTML = `<span style="color: #72767d; font-style: italic;">No bones selected</span>`;
        return;
    }

    bonesArray.forEach(bone => {
        const item = document.createElement('div');
        item.style.padding = "2px 0";
        item.innerText = `🦴 ${bone}`;
        container.appendChild(item);
    });
};

async function loadEditorConfig(lang) {
    try {
        const response = await fetch(`/locales/blocks.json-${lang}.json`);
        if (!response.ok) throw new Error("Could not fetch data configuration");

        const config = await response.json();

        config.blocks.forEach(blockData => {
            if (Blockly.Blocks[blockData.type]) {
                delete Blockly.Blocks[blockData.type];
            }
        });

        Blockly.defineBlocksWithJsonArray(config.blocks);

        let currentWorkspaceState = null;
        if (workspace) {
            currentWorkspaceState = Blockly.serialization.workspaces.save(workspace);
            workspace.dispose();
        }

        if (bevyCanvas) {
            bevyCanvas.addEventListener('contextmenu', (e) => { e.preventDefault(); });
            bevyCanvas.addEventListener('mousedown', () => { bevyCanvas.focus(); });
            bevyCanvas.addEventListener('keydown', (e) => {
                const keysToBlock = [' ', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Tab'];
                if (keysToBlock.includes(e.key)) { e.preventDefault(); }
            });
        }

        workspace = Blockly.inject('blockly-div', {
            toolbox: config.toolbox,
            scrollbars: true,
            trashcan: true,
            media: 'https://unpkg.com/blockly/media/'
        });

        if (currentWorkspaceState) {
            Blockly.serialization.workspaces.load(currentWorkspaceState, workspace);
        }
    } catch (err) {
        console.error("Failed to load JSON workspace config:", err);
    }
}

async function loadSceneAndPopulateUI() {
    try {
        let sceneData = [];


        const cachedScene = localStorage.getItem("waddlie_current_scene");

        if (cachedScene) {
            sceneData = JSON.parse(cachedScene);
            console.log("Loaded active scene layout from LocalStorage Cache:", sceneData);
        } else {
            // Fallback to disk if the cache is pristine and empty
            const response = await fetch('assets/scene.json');
            if (!response.ok) throw new Error("Failed to fetch scene file");
            sceneData = await response.json();
            console.log("Loaded initial scene file integration layout:", sceneData);
        }

        const entities = sceneData.filter(item => item.type === "Entity");
        const container = document.getElementById('entities-list-container');
        container.innerHTML = "";

        entities.forEach(entity => {
            const button = document.createElement('button');
            button.className = 'entity-btn';
            button.innerText = `ID ${entity.id}: ${entity.name}`;

            button.addEventListener('click', () => {
                document.querySelectorAll('.entity-btn').forEach(btn => btn.classList.remove('active'));
                button.classList.add('active');

                selectedEntityId = entity.id;
                console.log(`Focused Scope Target Entity ID: ${selectedEntityId}`);
                renderComponentsForEntity(entity);
            });
            container.appendChild(button);
        });
    } catch (err) {
        console.error("Error processing scene file integration UI layout:", err);
    }
}

function renderFileSystemExplorer() {
    const grid = document.getElementById('assets-file-grid');
    const breadcrumbs = document.getElementById('assets-breadcrumbs');
    if (!grid || !breadcrumbs) return;

    grid.innerHTML = "";

    if (currentFolderView === "root") {
        breadcrumbs.innerText = "Root /";

        // Render 'models' folder directory entry
        const folder = document.createElement('div');
        folder.style = "padding: 8px; background: #36393f; border-radius: 4px; cursor: pointer; color: #f1c40f; font-weight: bold; display: flex; align-items: center; gap: 6px; user-select: none;";
        folder.innerText = "models";
        folder.addEventListener('dblclick', () => {
            currentFolderView = "models";
            renderFileSystemExplorer();
        });
        grid.appendChild(folder);
    } else if (currentFolderView === "models") {
        breadcrumbs.innerText = "Root / models / ↩ (Double-click here to go back)";
        breadcrumbs.onclick = () => {
            currentFolderView = "root";
            renderFileSystemExplorer();
        };

        // Render localized File Upload Trigger Button


        // List uploaded assets within the virtual folder scope
        if (importedModelFileList.length === 0) {
            const emptyLabel = document.createElement('span');
            emptyLabel.style = "color: #72767d; font-style: italic; font-size: 11px; padding: 4px;";
            emptyLabel.innerText = "Folder empty";
            grid.appendChild(emptyLabel);
        } else {
            importedModelFileList.forEach(fileName => {
                const item = document.createElement('div');
                item.style = "padding: 4px 6px; background: #2f3136; border-radius: 4px; color: #fff; font-size: 12px; font-family: monospace;";
                item.innerText = `${fileName}`;
                grid.appendChild(item);
            });
        }
    }
}

function renderComponentsForEntity(entity) {
    const compContainer = document.getElementById('components-list-container');
    compContainer.innerHTML = "";

    const allComponents = [];
    // Build the Transform data proxy structure
    allComponents.push({
        type: "Transform",
        position_x: entity.position_x, position_y: entity.position_y, position_z: entity.position_z,
        rotation_x: entity.rotation_x, rotation_y: entity.rotation_y, rotation_z: entity.rotation_z,
        scale_x: entity.scale_x, scale_y: entity.scale_y, scale_z: entity.scale_z
    });

    if (entity.components && entity.components.length > 0) {
        entity.components.forEach(comp => allComponents.push(comp));
    }

    allComponents.forEach((comp, index) => {
        const compWrapper = document.createElement('div');
        compWrapper.style.marginBottom = "8px";

        const btn = document.createElement('button');
        btn.className = 'component-btn';
        btn.innerText = `${comp.type}`;

        const specsPanel = document.createElement('div');
        specsPanel.className = 'specs-panel';
        specsPanel.style.display = 'none';

        Object.keys(comp).forEach(key => {
            if (key === 'type') return;
            const specLine = document.createElement('div');
            specLine.className = 'spec-line';
            specLine.style.display = "flex";
            specLine.style.justifyContent = "space-between";
            specLine.style.alignItems = "center";
            specLine.style.marginBottom = "4px";

            const keySpan = document.createElement('span');
            keySpan.className = 'spec-key';
            keySpan.innerText = `${key}:`;

            let val = comp[key];
            let valueField;

            // Handle complex object arrays fallback
            if (typeof val === 'object' && val !== null) {
                valueField = document.createElement('span');
                valueField.className = 'spec-val';
                valueField.innerText = JSON.stringify(val);
            } else {
                // Create an editable input field
                valueField = document.createElement('input');
                valueField.className = 'spec-property-input';
                valueField.style.background = "#23272a";
                valueField.style.border = "1px solid #202225";
                valueField.style.color = "#fff";
                valueField.style.borderRadius = "4px";
                valueField.style.padding = "2px 6px";
                valueField.style.width = "60%";
                valueField.style.boxSizing = "border-box";

                // Enforce proper HTML inputs based on JSON types
                if (typeof val === 'number') {
                    valueField.type = 'number';
                    valueField.step = 'any';
                    valueField.value = val;
                } else {
                    valueField.type = 'text';
                    valueField.value = val !== undefined ? val : "";
                }

                // Track and process changes live
                valueField.addEventListener('change', (e) => {
                    const updatedVal = valueField.type === 'number' ? parseFloat(e.target.value) : e.target.value;

                    // 1. Maintain context within active DOM memory context
                    comp[key] = updatedVal;

                    // 2. Dispatch signals to Bevy WebAssembly core architecture
                    if (comp.type === "Transform") {
                        // Adapt "position_x" naming to match backend "nudge_x" format requirement
                        let transformType = key;
                        if (key.startsWith('position_')) {
                            transformType = key.replace('position_', 'nudge_');
                        }
                        updateEntityTransform(entity.id, transformType, updatedVal);
                    } else if (comp.type === "GltfModel" && key === "path") {
                        // Extract standalone filename if full paths are explicitly tracked
                        const cleanFileName = updatedVal.substring(updatedVal.lastIndexOf('/') + 1);
                        hotSwapGltfModel(entity.id, cleanFileName);
                    }

                    // 3. Persist modifications locally into the Cache System layer
                    const cachedScene = localStorage.getItem("waddlie_current_scene");
                    if (cachedScene) {
                        const sceneData = JSON.parse(cachedScene);
                        const targetEntity = sceneData.find(item => item.id === entity.id);
                        if (targetEntity) {
                            if (comp.type === "Transform") {
                                targetEntity[key] = updatedVal;
                            } else if (targetEntity.components) {
                                const targetComp = targetEntity.components.find(c => c.type === comp.type);
                                if (targetComp) targetComp[key] = updatedVal;
                            }
                            localStorage.setItem("waddlie_current_scene", JSON.stringify(sceneData));
                        }
                    }
                });
            }

            specLine.appendChild(keySpan);
            specLine.appendChild(valueField);
            specsPanel.appendChild(specLine);
        });

        // Add Rigging Machine context hooks safely
        if (comp.type === "GltfModel") {
            const setupRigBtn = document.createElement('button');
            setupRigBtn.innerText = "Set up Animation State Machine";
            setupRigBtn.style.width = "100%";
            setupRigBtn.style.marginTop = "6px";
            setupRigBtn.style.padding = "6px";
            setupRigBtn.style.background = "#7289da";
            setupRigBtn.style.color = "white";
            setupRigBtn.style.border = "none";
            setupRigBtn.style.borderRadius = "4px";
            setupRigBtn.style.cursor = "pointer";

            setupRigBtn.addEventListener('click', (e) => {
                e.stopPropagation();
                document.querySelector('.data-lists-panel').style.display = 'none';
                document.getElementById('rigging-setup-panel').style.display = 'block';
                wasmEngine.toggle_rigging_mode(selectedEntityId, true);
            });
            compWrapper.appendChild(setupRigBtn);
        }

        btn.addEventListener('click', () => {
            const isCurrentlyVisible = specsPanel.style.display === 'block';
            document.querySelectorAll('.specs-panel').forEach(p => p.style.display = 'none');
            document.querySelectorAll('.component-btn').forEach(b => b.classList.remove('active'));

            if (!isCurrentlyVisible) {
                specsPanel.style.display = 'block';
                btn.classList.add('active');
            }
        });

        compWrapper.appendChild(btn);
        compWrapper.appendChild(specsPanel);
        compContainer.appendChild(compWrapper);
    });
    renderFileSystemExplorer();


    // Track and process changes live
    valueField.addEventListener('change', (e) => {
        const updatedVal = valueField.type === 'number' ? parseFloat(e.target.value) : e.target.value;

        // 1. Maintain context within active DOM memory context
        comp[key] = updatedVal;


        handlePropertyChange(entity.id, key, updatedVal);

        // 2. Persist modifications locally into the Cache System layer
        const cachedScene = localStorage.getItem("waddlie_current_scene");
        if (cachedScene) {
            const sceneData = JSON.parse(cachedScene);
            const targetEntity = sceneData.find(item => item.id === entity.id);
            if (targetEntity) {
                if (comp.type === "Transform") {
                    targetEntity[key] = updatedVal;
                } else if (targetEntity.components) {
                    const targetComp = targetEntity.components.find(c => c.type === comp.type);
                    if (targetComp) targetComp[key] = updatedVal;
                }
                localStorage.setItem("waddlie_current_scene", JSON.stringify(sceneData));
            }
        }
    });
}



function handlePropertyChange(entityId, propertyName, newValue) {
    // Basic console print matching your requested format
    console.log(`entity id ${entityId} ${propertyName} = ${newValue}`);

    // --- IF/ELSE BRANCHES BASED ON PROPERTY NAME ---
    if (propertyName === "position_x" || propertyName === "position_y" || propertyName === "position_z") {
        const transformType = propertyName.replace("position_", "nudge_");
        updateEntityTransform(entityId, transformType, newValue);

    } else if (propertyName === "rotation_x" || propertyName === "rotation_y" || propertyName === "rotation_z") {
        updateEntityTransform(entityId, propertyName, newValue);

    } else if (propertyName === "scale_x" || propertyName === "scale_y" || propertyName === "scale_z") {
        updateEntityTransform(entityId, propertyName, newValue);

    } else if (propertyName === "path") {
        const cleanFileName = newValue.substring(newValue.lastIndexOf('/') + 1);
        hotSwapGltfModel(entityId, cleanFileName);

    } else {
        console.log(`Untracked custom property altered: ${propertyName}`);
    }
}




document.getElementById('btn-en').addEventListener('click', () => loadEditorConfig('en'));

function initializeApp() {
    const blocklyDiv = document.getElementById('blockly-div');
    const bevyCanvas = document.getElementById('bevy-canvas');

    if (!blocklyDiv) {
        setTimeout(initializeApp, 50);
        return;
    }

    console.log("DOM fully ready! Initializing editor interfaces...");
    loadEditorConfig('en');
    loadSceneAndPopulateUI();

    if (bevyCanvas) {
        bevyCanvas.addEventListener('contextmenu', (e) => { e.preventDefault(); });
        bevyCanvas.addEventListener('mousedown', () => { bevyCanvas.focus(); });
    }


    document.getElementById('btn-exit-rigging').addEventListener('click', () => {
        document.getElementById('rigging-setup-panel').style.display = 'none';
        document.querySelector('.data-lists-panel').style.display = 'flex';

        // Notify Bevy engine to shut down selection rendering hooks
        wasmEngine.toggle_rigging_mode(selectedEntityId, false);
    });

    document.getElementById('btn-make-group').addEventListener('click', () => {
        const groupNameInput = document.getElementById('bone-group-name');
        const groupName = groupNameInput.value.trim();

        if (!groupName) {
            alert("Please enter a name for your custom bone group!");
            return;
        }

        const boneElements = document.getElementById('selected-bones-list').querySelectorAll('div');
        const selectedBones = Array.from(boneElements).map(el => el.innerText.replace('🦴 ', ''));

        if (selectedBones.length === 0) {
            alert("Please select at least one bone in the 3D viewport!");
            return;
        }

        console.log(`Saving bone group: "${groupName}" containing bones:`, selectedBones);

        // Clear input and confirm selection
        groupNameInput.value = "";
        alert(`Successfully mapped bone group "${groupName}"!`);
    });
    renderFileSystemExplorer(); // Ensure the file system explorer is also refreshed
}


let currentImportedData = {
    fileName: "",
    arrayBuffer: null
};

// Expose a function Bevy can call back into once it inspects the GLB details
window.populateImportAnimationDropdown = function (animationsJson) {
    const animationNames = JSON.parse(animationsJson); // Array of strings from GLTF
    const dropdown = document.getElementById('import-animation-dropdown');

    // Reset dropdown container keeping default option
    dropdown.innerHTML = '<option value="__DEFAULT_POSE__">-- Default Pose (Static) --</option>';

    animationNames.forEach(name => {
        const option = document.createElement('option');
        option.value = name;
        option.innerText = name;
        dropdown.appendChild(option);
    });

    // Reveal config panel
    document.getElementById('model-importer-panel').style.display = 'block';
};

// UI Handling Initialization inside your setup hooks
const importBtn = document.getElementById('btn-import-glb');
const filePicker = document.getElementById('glb-file-picker');

importBtn.addEventListener('click', () => filePicker.click());

// --- Find your existing filePicker 'change' listener and insert the caching hook ---
// --- Find your existing filePicker 'change' listener and insert the tracking hook ---
filePicker.addEventListener('change', (e) => {
    const file = e.target.files[0];
    if (!file) return;

    currentImportedData.fileName = file.name;
    const reader = new FileReader();

    reader.onload = async function (event) {
        currentImportedData.arrayBuffer = event.target.result;
        const uint8Array = new Uint8Array(currentImportedData.arrayBuffer);

        // Store bytes globally matching the 'models/filename.glb' schema layout 
        uploadedAssetsMemoryCache.set(`models/${file.name}`, uint8Array);


        if (!importedModelFileList.includes(file.name)) {
            importedModelFileList.push(file.name);
        }

        // [Your existing WASM registration and entity spawning code layout follows...]
        console.log(`[JS -> WASM] Registering virtual asset path: 'models/${file.name}'`);
        wasmEngine.register_virtual_glb_asset(file.name, uint8Array);

        // ... (Keep your rest of the setup code exactly as it is) ...

        localStorage.setItem("waddlie_current_scene", JSON.stringify(sceneData));
        wasmEngine.spawn_imported_entity(JSON.stringify(newEntity));


        loadSceneAndPopulateUI();
        renderFileSystemExplorer();
    };
    reader.readAsArrayBuffer(file);
});


document.getElementById('btn-export-project').addEventListener('click', async () => {
    console.log("Packaging active scene assets into Game.zip...");

    // 1. Resolve final state configuration (prioritize live LocalStorage memory state, fallback to disk layout)
    let currentSceneData = [];
    const cachedScene = localStorage.getItem("waddlie_current_scene");

    if (cachedScene && cachedScene.trim() !== "") {
        currentSceneData = JSON.parse(cachedScene);
    } else {
        try {
            const response = await fetch('assets/scene.json');
            if (response.ok) currentSceneData = await response.json();
        } catch (e) {
            console.error("Could not trace primary fallback disk directory config:", e);
        }
    }

    // 2. Instantiate JSZip virtual layout container
    const zip = new JSZip();

    // Create 'assets/' root and 'assets/models/' structural children trees
    const assetsFolder = zip.folder("assets");
    const modelsFolder = assetsFolder.folder("models");

    // 3. Insert the fresh scene metadata configuration into 'assets/scene.json'
    const sceneJsonString = JSON.stringify(currentSceneData, null, 2);
    assetsFolder.file("scene.json", sceneJsonString);

    // 4. Iterate through active entities to extract and append respective GLB files
    currentSceneData.forEach(item => {
        if (item.type === "Entity" && item.components) {
            item.components.forEach(comp => {
                if (comp.type === "GltfModel" && comp.path) {
                    // Extract baseline filename from 'models/example.glb' or 'example.glb'
                    const fullPath = comp.path;
                    const fileName = fullPath.substring(fullPath.lastIndexOf('/') + 1);

                    // Check if we have the binary raw buffer stored in our operational browser cache
                    if (uploadedAssetsMemoryCache.has(fullPath)) {
                        const binaryData = uploadedAssetsMemoryCache.get(fullPath);
                        modelsFolder.file(fileName, binaryData);
                        console.log(`Bundled uploaded model binary asset: "assets/models/${fileName}"`);
                    } else {
                        console.warn(`Model source "${fullPath}" was present at initial boot but wasn't newly uploaded this session. Skipping binary file embedding.`);
                    }
                }
            });
        }
    });

    // 5. Generate binary blob compression archive and trigger native browser prompt download pipeline
    zip.generateAsync({ type: "blob" }).then((content) => {
        const downloadAnchor = document.createElement("a");
        downloadAnchor.href = URL.createObjectURL(content);
        downloadAnchor.download = "Game.zip";

        document.body.appendChild(downloadAnchor);
        downloadAnchor.click();

        // Cleanup document lifecycle
        document.body.removeChild(downloadAnchor);
        URL.revokeObjectURL(downloadAnchor.href);
        console.log("Successfully downloaded Game.zip container archive!");
    }).catch(err => {
        console.error("Failed to compress project folders layout into Zip file bundle:", err);
    });
});

// --- Add this to the very bottom of app.js ---
bevyCanvas.addEventListener('click', (e) => {
    if (e.target === bevyCanvas) {
        selectedEntityId = null;

        // Remove selection style from all elements in the hierarchy
        document.querySelectorAll('.hierarchy-item').forEach(el => el.classList.remove('selected'));

        // Clear selection in Inspector
        renderInspector();
    }
});




window.refreshJsEntityList = function () {
    console.log("Bevy Engine requested a UI hierarchy synchronization update.");
    loadSceneAndPopulateUI();

};



if (document.readyState === 'loading') {
    window.addEventListener('DOMContentLoaded', initializeApp);
} else {
    initializeApp();
}