/**
 * Babylon-Chunks Route Handler
 * 
 * This endpoint demonstrates the Wave Function Collapse (WFC) algorithm
 * visualized in 3D using BabylonJS. It generates a hexagonal grid of 3D tiles
 * using mesh instancing for optimal performance.
 * 
 * **Key Features:**
 * - WFC algorithm implemented in Rust WASM
 * - 5 different 3D tile types
 * - GLB model loading for hex tiles (see TILE_CONFIG for dimensions, pointy-top orientation)
 * - Mesh instancing for performance
 * - Babylon 2D UI for controls
 * - Fullscreen support
 */

import type { LayoutConstraints } from '../types';
import { WasmLoadError, WasmInitError } from '../wasm/types';
import { WasmManager } from './babylon-chunks/wasmManagement';
import { PatternCacheManager } from './babylon-chunks/dbManagement';
import { LlmManager } from './babylon-chunks/llmManagement';
import { CanvasManager } from './babylon-chunks/canvasManagement';
import { generateLayoutFromText, constraintsToPreConstraints } from './babylon-chunks/layoutGeneration';
import { WorldMap } from './babylon-chunks/chunkManagement';
import { TestManager } from './babylon-chunks/testManagement';
import { TILE_CONFIG } from './babylon-chunks/canvasManagement';

/**
 * Runtime Configuration
 */
type ConfigMode = 'normal' | 'test';

const CONFIG: { mode: ConfigMode } = {
  mode: 'test',
};

/**
 * Initialize the babylon-chunks route
 */
export const init = async (): Promise<void> => {
  const errorEl = document.getElementById('error');
  const canvasEl = document.getElementById('renderCanvas');
  const systemLogsContentEl = document.getElementById('systemLogsContent');
  
  if (!canvasEl) {
    throw new Error('renderCanvas element not found');
  }
  
  if (!(canvasEl instanceof HTMLCanvasElement)) {
    throw new Error('renderCanvas element is not an HTMLCanvasElement');
  }
  
  const canvas = canvasEl;
  
  // Prevent wheel events from scrolling the page when over canvas
  // CSS overscroll-behavior doesn't work for wheel events, need JavaScript
  canvas.addEventListener('wheel', (event) => {
    // Only prevent if the event is actually on the canvas
    if (event.target === canvas) {
      event.preventDefault();
    }
  }, { passive: false });
  
  // Setup logging
  let addLogEntry: ((message: string, type?: 'info' | 'success' | 'warning' | 'error') => void) | null = null;
  if (systemLogsContentEl) {
    addLogEntry = (message: string, type: 'info' | 'success' | 'warning' | 'error' = 'info'): void => {
      const timestamp = new Date().toLocaleTimeString();
      const logEntry = document.createElement('div');
      logEntry.className = `log-entry ${type}`;
      logEntry.textContent = `[${timestamp}] ${message}`;
      systemLogsContentEl.appendChild(logEntry);
      systemLogsContentEl.scrollTop = systemLogsContentEl.scrollHeight;
    };
  }

  // Initialize modules with dependency injection
  const wasmManager = new WasmManager();
  const llmManager = new LlmManager(addLogEntry ?? undefined);
  const patternCache = new PatternCacheManager(
    addLogEntry ?? undefined,
    (text: string) => llmManager.generateEmbedding(text)
  );
  
  // Get initial rings value from dropdown (default 5)
  const initialRingsSelectEl = document.getElementById('ringsSelect');
  let initialRings = 5; // Default value
  if (initialRingsSelectEl && initialRingsSelectEl instanceof HTMLSelectElement) {
    const selectedRings = Number.parseInt(initialRingsSelectEl.value, 10);
    if (!Number.isNaN(selectedRings) && selectedRings >= 0 && selectedRings <= 50) {
      initialRings = selectedRings;
    }
  }
  
  const canvasManager = new CanvasManager(wasmManager, addLogEntry ?? undefined, undefined, CONFIG.mode === 'test');
  canvasManager.setCurrentRings(initialRings);

  // Set up pre-constraints generation function for canvas manager
  canvasManager.setGeneratePreConstraintsFn((constraints: LayoutConstraints) => {
    const wasmModule = wasmManager.getModule();
    if (!wasmModule) {
      return [];
    }
    return constraintsToPreConstraints(
      constraints,
      wasmModule,
      canvasManager.getCurrentRings(),
      (rings) => canvasManager.setCurrentRings(rings),
      addLogEntry ?? undefined
    );
  });

  // Initialize pattern cache in background (non-blocking)
  void patternCache.initializeCommonPatterns().catch((error) => {
    const errorMsg = error instanceof Error ? error.message : 'Unknown error';
    if (addLogEntry) {
      addLogEntry(`Pattern cache initialization failed: ${errorMsg}`, 'warning');
    }
  });
  
  // Initialize WASM module
  try {
    await wasmManager.initialize();
    
    // Log WASM version for debugging and cache verification
    const wasmModule = wasmManager.getModule();
    if (wasmModule && addLogEntry) {
      const wasmVersion = wasmModule.get_wasm_version();
      addLogEntry(`WASM module version: ${wasmVersion}`, 'info');
    }
  } catch (error) {
    if (errorEl) {
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      if (error instanceof WasmLoadError) {
        errorEl.textContent = `Failed to load WASM module: ${errorMsg}`;
      } else if (error instanceof WasmInitError) {
        errorEl.textContent = `WASM module initialization failed: ${errorMsg}`;
      } else if (error instanceof Error) {
        errorEl.textContent = `Error: ${errorMsg}`;
        if (error.stack) {
          errorEl.textContent += `\n\nStack: ${error.stack}`;
        }
        if ('cause' in error && error.cause) {
          const causeMsg = error.cause instanceof Error 
            ? error.cause.message 
            : typeof error.cause === 'string' 
              ? error.cause 
              : JSON.stringify(error.cause);
          errorEl.textContent += `\n\nCause: ${causeMsg}`;
        }
      } else {
        errorEl.textContent = 'Unknown error loading WASM module';
      }
    }
    throw error;
  }
  
  // Initialize canvas manager
  await canvasManager.initialize(canvas);
  
  // Set initial background color from dropdown
  const initialBackgroundColorSelectEl = document.getElementById('backgroundColorSelect');
  if (initialBackgroundColorSelectEl && initialBackgroundColorSelectEl instanceof HTMLSelectElement) {
    canvasManager.setBackgroundColor(initialBackgroundColorSelectEl.value);
  }
  
  // Create map for chunk management
  const worldMap = new WorldMap();
  
  // Create origin chunk at (0, 0)
  const originPosition = { q: 0, r: 0 };
  const originChunk = worldMap.createChunk(
    originPosition,
    canvasManager.getCurrentRings(),
    TILE_CONFIG.hexSize
  );
  
  // Compute neighbors for origin chunk (already computed in constructor)
  if (addLogEntry) {
    const neighbors = originChunk.getNeighbors();
    addLogEntry(`Origin chunk created at (0, 0) with ${neighbors.length} neighbors`, 'info');
    for (const neighbor of neighbors) {
      addLogEntry(`Origin chunk neighbor: (${neighbor.q}, ${neighbor.r})`, 'info');
    }
  }
  
  // Run tests if mode is test
  if (CONFIG.mode === 'test') {
    const testManager = new TestManager(worldMap, addLogEntry ?? undefined);
    testManager.testOriginChunkNeighborsWithCreation(
      canvasManager.getCurrentRings(),
      TILE_CONFIG.hexSize
    );
  }
  
  // Set map in canvas manager for rendering
  canvasManager.setMap(worldMap);
  
  // Initial render
  canvasManager.renderGrid();
  
  // Text input and generate button (HTML elements)
  const promptInputEl = document.getElementById('layoutPromptInput');
  const generateFromTextBtn = document.getElementById('generateFromTextBtn');
  const modelStatusEl = document.getElementById('modelStatus');

  if (generateFromTextBtn && promptInputEl) {
    generateFromTextBtn.addEventListener('click', () => {
      const prompt = promptInputEl instanceof HTMLInputElement ? promptInputEl.value.trim() : '';
      if (prompt) {
        generateLayoutFromText(
          prompt,
          wasmManager,
          llmManager,
          patternCache,
          canvasManager,
          (constraints?: LayoutConstraints) => canvasManager.renderGrid(constraints),
          errorEl,
          modelStatusEl,
          addLogEntry ?? undefined
        ).catch((error) => {
          const errorMsg = error instanceof Error ? error.message : 'Unknown error';
          if (errorEl) {
            errorEl.textContent = `Error: ${errorMsg}`;
          }
        });
      }
    });
  }

  /**
   * Reinitialize everything - called when rings or mode changes
   */
  const reinitialize = async (): Promise<void> => {
    try {
      // Get current rings value from dropdown
      const ringsSelectEl = document.getElementById('ringsSelect');
      let currentRings = canvasManager.getCurrentRings();
      if (ringsSelectEl && ringsSelectEl instanceof HTMLSelectElement) {
        const selectedRings = Number.parseInt(ringsSelectEl.value, 10);
        if (!Number.isNaN(selectedRings) && selectedRings >= 0 && selectedRings <= 50) {
          currentRings = selectedRings;
        }
      }

      // Clear system logs
      if (systemLogsContentEl) {
        systemLogsContentEl.innerHTML = '';
      }

      // Dispose of old canvas manager
      canvasManager.dispose();

      // Clear WASM state
      const wasmModule = wasmManager.getModule();
      if (wasmModule) {
        wasmModule.clear_layout();
        wasmModule.clear_pre_constraints();
      }

      // Create new canvas manager with updated test mode
      const newCanvasManager = new CanvasManager(wasmManager, addLogEntry ?? undefined, undefined, CONFIG.mode === 'test');
      
      // Set the rings value before initialization
      newCanvasManager.setCurrentRings(currentRings);

      // Set up pre-constraints generation function
      newCanvasManager.setGeneratePreConstraintsFn((constraints: LayoutConstraints) => {
        const module = wasmManager.getModule();
        if (!module) {
          return [];
        }
        return constraintsToPreConstraints(
          constraints,
          module,
          newCanvasManager.getCurrentRings(),
          (rings) => newCanvasManager.setCurrentRings(rings),
          addLogEntry ?? undefined
        );
      });

      // Initialize canvas manager
      await newCanvasManager.initialize(canvas);

      // Set background color from dropdown
      const backgroundColorSelectEl = document.getElementById('backgroundColorSelect');
      if (backgroundColorSelectEl && backgroundColorSelectEl instanceof HTMLSelectElement) {
        newCanvasManager.setBackgroundColor(backgroundColorSelectEl.value);
      }

      // Create new map for chunk management
      const newWorldMap = new WorldMap();

      // Create origin chunk at (0, 0) with current rings value
      const originPosition = { q: 0, r: 0 };
      const originChunk = newWorldMap.createChunk(
        originPosition,
        currentRings,
        TILE_CONFIG.hexSize
      );

      // Compute neighbors for origin chunk
      if (addLogEntry) {
        const neighbors = originChunk.getNeighbors();
        addLogEntry(`Origin chunk created at (0, 0) with ${neighbors.length} neighbors (rings: ${currentRings})`, 'info');
        for (const neighbor of neighbors) {
          addLogEntry(`Origin chunk neighbor: (${neighbor.q}, ${neighbor.r})`, 'info');
        }
      }

      // Run tests if mode is test - use current rings value
      if (CONFIG.mode === 'test') {
        const testManager = new TestManager(newWorldMap, addLogEntry ?? undefined);
        testManager.testOriginChunkNeighborsWithCreation(
          currentRings,
          TILE_CONFIG.hexSize
        );
      }

      // Set map in canvas manager for rendering
      newCanvasManager.setMap(newWorldMap);

      // Initial render
      newCanvasManager.renderGrid();

      // Update the canvasManager reference
      // Note: We can't reassign const, so we'll need to update the handlers
      // For now, we'll store it in a way that allows updates
      Object.assign(canvasManager, newCanvasManager);
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      if (errorEl) {
        errorEl.textContent = `Reinitialization error: ${errorMsg}`;
      }
      if (addLogEntry) {
        addLogEntry(`Reinitialization error: ${errorMsg}`, 'error');
      }
    }
  };

  // Rings dropdown handler
  const ringsSelectEl = document.getElementById('ringsSelect');
  if (ringsSelectEl && ringsSelectEl instanceof HTMLSelectElement) {
    // Set initial value to currentRings (default 5)
    ringsSelectEl.value = canvasManager.getCurrentRings().toString();
    
    ringsSelectEl.addEventListener('change', () => {
      const selectedRings = Number.parseInt(ringsSelectEl.value, 10);
      if (!Number.isNaN(selectedRings) && selectedRings >= 0 && selectedRings <= 50) {
        // Update rings in canvas manager
        canvasManager.setCurrentRings(selectedRings);
        
        // Reinitialize everything
        void reinitialize();
      }
    });
  }

  // Runtime mode dropdown handler
  const runtimeModeSelectEl = document.getElementById('runtimeModeSelect');
  if (runtimeModeSelectEl && runtimeModeSelectEl instanceof HTMLSelectElement) {
    // Set initial value to current mode
    runtimeModeSelectEl.value = CONFIG.mode;
    
    runtimeModeSelectEl.addEventListener('change', () => {
      const selectedMode = runtimeModeSelectEl.value;
      if (selectedMode === 'normal' || selectedMode === 'test') {
        // Update CONFIG mode
        CONFIG.mode = selectedMode;
        
        // Reinitialize everything
        void reinitialize();
      }
    });
  }

  // Background color dropdown handler
  const backgroundColorSelectEl = document.getElementById('backgroundColorSelect');
  if (backgroundColorSelectEl && backgroundColorSelectEl instanceof HTMLSelectElement) {
    // Set initial background color
    canvasManager.setBackgroundColor(backgroundColorSelectEl.value);
    
    backgroundColorSelectEl.addEventListener('change', () => {
      const selectedColor = backgroundColorSelectEl.value;
      // Update background color immediately (no need to reinitialize)
      canvasManager.setBackgroundColor(selectedColor);
    });
  }
};
