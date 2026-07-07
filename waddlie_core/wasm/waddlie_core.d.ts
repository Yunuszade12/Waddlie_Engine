/* tslint:disable */
/* eslint-disable */

export function order_benvy(entity_id: number, command: string, _value_str: string, value_num: number): void;

export function register_virtual_glb_asset(file_name: string, file_bytes: Uint8Array): void;

export function spawn_imported_entity(entity_json: string): void;

export function toggle_rigging_mode(entity_json_id: number, activate: boolean): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly main: (a: number, b: number) => number;
    readonly order_benvy: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly register_virtual_glb_asset: (a: number, b: number, c: number, d: number) => void;
    readonly spawn_imported_entity: (a: number, b: number) => void;
    readonly toggle_rigging_mode: (a: number, b: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h1c2d65ca8bdd7da2: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen__convert__closures_____invoke__hdda7b6e24b219676: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h28a46cc16efee1bb: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h1a86770ad3c91a64: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h28a46cc16efee1bb_4: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h28a46cc16efee1bb_5: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h28a46cc16efee1bb_6: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h28a46cc16efee1bb_7: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h28a46cc16efee1bb_8: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h28a46cc16efee1bb_9: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h28a46cc16efee1bb_10: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hee949aa9b4749057: (a: number, b: number, c: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__h2d0fb4c3456fe2a3: (a: number, b: number) => void;
    readonly wasm_bindgen__convert__closures_____invoke__hf294e26e283a81bc: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
