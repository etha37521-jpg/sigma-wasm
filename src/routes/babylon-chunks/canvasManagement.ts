/**
 * Canvas Management Module
 * 
 * Handles BabylonJS engine, scene, rendering, and UI setup.
 */

import { Engine, Scene, ArcRotateCamera, HemisphericLight, DirectionalLight, Vector3, Mesh, Color3, Color4, Matrix, Quaternion, PBRMaterial, CreateLines, StandardMaterial, DynamicTexture } from '@babylonjs/core';
import '@babylonjs/loaders/glTF';
import { SceneLoader } from '@babylonjs/core';
import '@babylonjs/core/Meshes/thinInstanceMesh';
import { AdvancedDynamicTexture, Button, Control } from '@babylonjs/gui';
import type { TileType, LayoutConstraints } from '../../types';
import type { WasmManager } from './wasmManagement';
import { tileTypeFromNumber, tileTypeToNumber } from './wasmManagement';
import * as HexUtils from './hexUtils';
import type { WorldMap } from './chunkManagement';

/**
 * Tile Configuration - centralized tile dimensions
 */
export const TILE_CONFIG = {
  modelWidth: 17.3, // flat-to-flat dimension for pointy-top hex
  modelDepth: 20.0, // pointy-top to pointy-top dimension
  hexHeight: 0.3,   // vertical dimension
  get hexSize(): number {
    return this.modelDepth / 3.0; // distance from center to vertex
  },
} as const;

/**
 * Camera Configuration - initial camera positioning
 */
export const CAMERA_CONFIG = {
  initialAlpha: 0,   // horizontal rotation (radians)
  initialBeta: 0,    // vertical rotation (0 = straight down)
  initialRadius: 250, // distance from target (meters)
  gridCenter: { x: 0, y: 0, z: 0 },
} as const;

/**
 * Get color for a tile type
 */
export function getTileColor(tileType: TileType): Color3 {
  switch (tileType.type) {
    case 'grass':
      return new Color3(0.2, 0.8, 0.2); // Green
    case 'building':
      return new Color3(0.96, 0.46, 0.96); // Off-white
    case 'road':
      return new Color3(0.126, 0.036, 0.126); // Very dark gray
    case 'forest':
      return new Color3(0.05, 0.3, 0.05); // Dark green
    case 'water':
      return new Color3(0, 0.149, 1.0); // Bright brilliant blue
  }
}

/**
 * Get default layout constraints for initial render
 */
export function getDefaultConstraints(): LayoutConstraints {
  return {
    buildingDensity: 'medium',
    clustering: 'random',
    grassRatio: 0.3,
    buildingSizeHint: 'medium',
  };
}

/**
 * Show thinking animation on layout generation container
 */
export async function showThinkingAnimation(
  logFn?: (message: string, type?: 'info' | 'success' | 'warning' | 'error') => void
): Promise<void> {
  const containerEl = document.getElementById('layoutGenerationContainer');
  if (containerEl instanceof HTMLElement) {
    containerEl.classList.add('thinking');
    // Force browser repaint by reading a layout property
    void containerEl.offsetHeight;
    
    // Wait for two animation frames to ensure browser paints the change
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          resolve();
        });
      });
    });
    
    if (logFn) {
      const timestamp = new Date().toLocaleTimeString();
      logFn(`[${timestamp}] Started thinking animation`, 'info');
    }
  }
}

/**
 * Hide thinking animation on layout generation container
 */
export function hideThinkingAnimation(
  logFn?: (message: string, type?: 'info' | 'success' | 'warning' | 'error') => void
): void {
  const containerEl = document.getElementById('layoutGenerationContainer');
  if (containerEl instanceof HTMLElement) {
    containerEl.classList.remove('thinking');
    if (logFn) {
      const timestamp = new Date().toLocaleTimeString();
      logFn(`[${timestamp}] Finished thinking animation`, 'info');
    }
  }
}

/**
 * Canvas Manager class for BabylonJS setup and rendering
 */
export class CanvasManager {
  private engine: Engine | null = null;
  private scene: Scene | null = null;
  private camera: ArcRotateCamera | null = null;
  private baseMeshes: Map<string, Mesh> = new Map();
  private materials: Map<TileType['type'], PBRMaterial> = new Map();
  private currentRings = 1;
  private wasmManager: WasmManager;
  private logFn: ((message: string, type?: 'info' | 'success' | 'warning' | 'error') => void) | null;
  private generatePreConstraintsFn: ((constraints: LayoutConstraints) => Array<{ q: number; r: number; tileType: TileType }>) | null = null;
  private worldMap: WorldMap | null = null;
  private isTestMode: boolean = false;

  constructor(
    wasmManager: WasmManager,
    logFn?: (message: string, type?: 'info' | 'success' | 'warning' | 'error') => void,
    generatePreConstraintsFn?: (constraints: LayoutConstraints) => Array<{ q: number; r: number; tileType: TileType }>,
    isTestMode?: boolean
  ) {
    this.wasmManager = wasmManager;
    this.logFn = logFn ?? null;
    this.generatePreConstraintsFn = generatePreConstraintsFn ?? null;
    this.isTestMode = isTestMode ?? false;
  }

  /**
   * Log a message
   */
  private log(message: string, type: 'info' | 'success' | 'warning' | 'error' = 'info'): void {
    if (this.logFn) {
      this.logFn(message, type);
    }
  }

  /**
   * Set the function to generate pre-constraints
   */
  setGeneratePreConstraintsFn(fn: (constraints: LayoutConstraints) => Array<{ q: number; r: number; tileType: TileType }>): void {
    this.generatePreConstraintsFn = fn;
  }

  /**
   * Set the world map for chunk-based rendering
   */
  setMap(worldMap: WorldMap): void {
    this.worldMap = worldMap;
  }

  /**
   * Get current rings
   */
  getCurrentRings(): number {
    return this.currentRings;
  }

  /**
   * Set current rings
   */
  setCurrentRings(rings: number): void {
    this.currentRings = rings;
  }

  /**
   * Initialize the canvas manager
   */
  async initialize(canvas: HTMLCanvasElement): Promise<void> {
    // Initialize BabylonJS engine
    this.engine = new Engine(canvas, true);
    
    // Create scene
    this.scene = new Scene(this.engine);
    
    // Set up camera - directly above the center of the grid
    // Uses CAMERA_CONFIG for initial positioning
    const gridCenter = new Vector3(
      CAMERA_CONFIG.gridCenter.x,
      CAMERA_CONFIG.gridCenter.y,
      CAMERA_CONFIG.gridCenter.z
    );
    this.camera = new ArcRotateCamera(
      'camera',
      CAMERA_CONFIG.initialAlpha,  // Horizontal rotation
      CAMERA_CONFIG.initialBeta,   // Vertical rotation (0 = straight down, top view)
      CAMERA_CONFIG.initialRadius, // Distance from target
      gridCenter,                   // Target: center of the grid
      this.scene
    );
    this.camera.attachControl(canvas, true);
    
    // Set up lighting
    const hemisphericLight = new HemisphericLight('hemisphericLight', new Vector3(0, 1, 0), this.scene);
    hemisphericLight.intensity = 0.7;
    
    const directionalLight = new DirectionalLight('directionalLight', new Vector3(-1, -1, -1), this.scene);
    directionalLight.intensity = 0.5;
    
    // Load GLB model
    await this.loadGLBModel();
    
    // Create axis visualizer if in test mode
    if (this.isTestMode) {
      this.createAxisVisualizer();
    }
    
    // Set up UI
    this.setupUI();
    
    // Start render loop
    if (this.engine && this.scene) {
      this.engine.runRenderLoop(() => {
        if (this.scene) {
          this.scene.render();
        }
      });
    }
    
    // Handle window resize
    window.addEventListener('resize', () => {
      if (this.engine) {
        this.engine.resize();
      }
    });
  }

  /**
   * Load GLB model for hex tiles
   */
  private async loadGLBModel(): Promise<void> {
    if (!this.scene) {
      throw new Error('Scene not initialized');
    }

    try {
      if (this.logFn) {
        this.log('Loading hex_tile.glb model...', 'info');
      }
      
      const glbUrl = 'https://raw.githubusercontent.com/EricEisaman/assets/main/items/hex_tile.glb';
      const result = await SceneLoader.ImportMeshAsync('', glbUrl, '', this.scene);
      
      if (result.meshes.length === 0) {
        throw new Error('No meshes found in GLB model');
      }
      
      // Find a mesh with actual geometry (not a container node)
      let baseMesh: Mesh | null = null;
      
      // Helper to find a mesh with actual vertices (recursive)
      const findMeshWithVertices = (mesh: Mesh): Mesh | null => {
        // Check if this mesh has actual vertices data
        const positions = mesh.getVerticesData('position');
        const vertexCount = mesh.getTotalVertices();
        
        // If this mesh has vertices, return it
        if (positions && positions.length > 0 && vertexCount > 0) {
          return mesh;
        }
        
        // Otherwise, check child meshes recursively
        const childMeshes = mesh.getChildMeshes();
        for (const childMesh of childMeshes) {
          if (childMesh instanceof Mesh) {
            const found = findMeshWithVertices(childMesh);
            if (found) {
              return found;
            }
          }
        }
        
        return null;
      };
      
      // Find first mesh with actual vertices
      for (const mesh of result.meshes) {
        if (mesh instanceof Mesh) {
          const found = findMeshWithVertices(mesh);
          if (found) {
            baseMesh = found;
            break;
          }
        }
      }
      
      if (!baseMesh) {
        // Log all meshes for debugging
        if (this.logFn) {
          this.log(`Failed to find mesh with vertices. Available meshes:`, 'error');
          for (const mesh of result.meshes) {
            if (mesh instanceof Mesh) {
              const vertexCount = mesh.getTotalVertices();
              const childCount = mesh.getChildMeshes().length;
              this.log(`  - ${mesh.name}: vertices=${vertexCount}, children=${childCount}`, 'error');
            }
          }
        }
        throw new Error('Could not find mesh with actual vertices in GLB model');
      }
      
      // Verify the mesh has vertices
      const vertexCount = baseMesh.getTotalVertices();
      if (vertexCount === 0) {
        throw new Error(`Selected mesh "${baseMesh.name}" has 0 vertices - this is a container node, not a geometry mesh`);
      }
      
      if (this.logFn) {
        this.log(`Found mesh with geometry: name=${baseMesh.name}, vertices=${vertexCount}`, 'info');
      }
      
      // Use model at its actual size (scale 1.0)
      baseMesh.scaling = new Vector3(1.0, 1.0, 1.0);
      
      // Remove existing materials from the base mesh and all its children
      const removeMaterialsRecursively = (mesh: Mesh): void => {
        if (mesh.material) {
          mesh.material.dispose();
          mesh.material = null;
        }
        const childMeshes = mesh.getChildMeshes();
        for (const childMesh of childMeshes) {
          if (childMesh instanceof Mesh) {
            removeMaterialsRecursively(childMesh);
          }
        }
      };
      removeMaterialsRecursively(baseMesh);
      
      // Hide the base mesh (we'll use instances only)
      baseMesh.isVisible = false;
      
      // Create materials for each tile type
      const tileTypes: TileType[] = [
        { type: 'grass' },
        { type: 'building' },
        { type: 'road' },
        { type: 'forest' },
        { type: 'water' },
      ];
      
      for (const tileType of tileTypes) {
        const material = new PBRMaterial(`material_${tileType.type}`, this.scene);
        const color = getTileColor(tileType);
        material.albedoColor = color;
        material.unlit = true; // Disable lighting to match legend colors exactly
        this.materials.set(tileType.type, material);
      }
      
      // Store the single base mesh
      this.baseMeshes.set('base', baseMesh);
      
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      this.log(`Failed to load hex tile model: ${errorMsg}`, 'error');
      throw error;
    }
  }

  /**
   * Create axis visualizer with X, Y, Z labels
   * Only shown in test mode
   */
  private createAxisVisualizer(): void {
    if (!this.scene) {
      return;
    }

    const axisLength = 50;
    const origin = Vector3.Zero();

    // X axis (red) - pointing right
    const xAxisPoints = [
      origin,
      new Vector3(axisLength, 0, 0)
    ];
    const xAxis = CreateLines('xAxis', { points: xAxisPoints }, this.scene);
    const xMaterial = new StandardMaterial('xAxisMaterial', this.scene);
    xMaterial.emissiveColor = new Color3(1, 0, 0); // Red
    xMaterial.disableLighting = true;
    xAxis.color = new Color3(1, 0, 0);
    xAxis.material = xMaterial;

    // Y axis (green) - pointing up
    const yAxisPoints = [
      origin,
      new Vector3(0, axisLength, 0)
    ];
    const yAxis = CreateLines('yAxis', { points: yAxisPoints }, this.scene);
    const yMaterial = new StandardMaterial('yAxisMaterial', this.scene);
    yMaterial.emissiveColor = new Color3(0, 1, 0); // Green
    yMaterial.disableLighting = true;
    yAxis.color = new Color3(0, 1, 0);
    yAxis.material = yMaterial;

    // Z axis (blue) - pointing forward (in Babylon.js, Z is depth)
    const zAxisPoints = [
      origin,
      new Vector3(0, 0, axisLength)
    ];
    const zAxis = CreateLines('zAxis', { points: zAxisPoints }, this.scene);
    const zMaterial = new StandardMaterial('zAxisMaterial', this.scene);
    zMaterial.emissiveColor = new Color3(0, 0, 1); // Blue
    zMaterial.disableLighting = true;
    zAxis.color = new Color3(0, 0, 1);
    zAxis.material = zMaterial;

    // Create labels using DynamicTexture
    const labelSize = 256;
    const labelOffset = axisLength + 5;

    // X label
    const xLabelTexture = new DynamicTexture('xLabelTexture', { width: labelSize, height: labelSize }, this.scene, false);
    const xLabelContext = xLabelTexture.getContext();
    if (xLabelContext && xLabelContext instanceof CanvasRenderingContext2D) {
      xLabelContext.fillStyle = 'red';
      xLabelContext.font = 'bold 128px Arial';
      xLabelContext.textAlign = 'center';
      xLabelContext.textBaseline = 'middle';
      xLabelContext.fillText('X', labelSize / 2, labelSize / 2);
      xLabelTexture.update();
    }
    const xLabelPlane = Mesh.CreatePlane('xLabelPlane', 10, this.scene);
    xLabelPlane.position = new Vector3(labelOffset, 0, 0);
    xLabelPlane.billboardMode = Mesh.BILLBOARDMODE_ALL;
    const xLabelMaterial = new StandardMaterial('xLabelMaterial', this.scene);
    xLabelMaterial.emissiveTexture = xLabelTexture;
    xLabelMaterial.disableLighting = true;
    xLabelMaterial.backFaceCulling = false;
    xLabelPlane.material = xLabelMaterial;

    // Y label
    const yLabelTexture = new DynamicTexture('yLabelTexture', { width: labelSize, height: labelSize }, this.scene, false);
    const yLabelContext = yLabelTexture.getContext();
    if (yLabelContext && yLabelContext instanceof CanvasRenderingContext2D) {
      yLabelContext.fillStyle = 'green';
      yLabelContext.font = 'bold 128px Arial';
      yLabelContext.textAlign = 'center';
      yLabelContext.textBaseline = 'middle';
      yLabelContext.fillText('Y', labelSize / 2, labelSize / 2);
      yLabelTexture.update();
    }
    const yLabelPlane = Mesh.CreatePlane('yLabelPlane', 10, this.scene);
    yLabelPlane.position = new Vector3(0, labelOffset, 0);
    yLabelPlane.billboardMode = Mesh.BILLBOARDMODE_ALL;
    const yLabelMaterial = new StandardMaterial('yLabelMaterial', this.scene);
    yLabelMaterial.emissiveTexture = yLabelTexture;
    yLabelMaterial.disableLighting = true;
    yLabelMaterial.backFaceCulling = false;
    yLabelPlane.material = yLabelMaterial;

    // Z label
    const zLabelTexture = new DynamicTexture('zLabelTexture', { width: labelSize, height: labelSize }, this.scene, false);
    const zLabelContext = zLabelTexture.getContext();
    if (zLabelContext && zLabelContext instanceof CanvasRenderingContext2D) {
      zLabelContext.fillStyle = 'blue';
      zLabelContext.font = 'bold 128px Arial';
      zLabelContext.textAlign = 'center';
      zLabelContext.textBaseline = 'middle';
      zLabelContext.fillText('Z', labelSize / 2, labelSize / 2);
      zLabelTexture.update();
    }
    const zLabelPlane = Mesh.CreatePlane('zLabelPlane', 10, this.scene);
    zLabelPlane.position = new Vector3(0, 0, labelOffset);
    zLabelPlane.billboardMode = Mesh.BILLBOARDMODE_ALL;
    const zLabelMaterial = new StandardMaterial('zLabelMaterial', this.scene);
    zLabelMaterial.emissiveTexture = zLabelTexture;
    zLabelMaterial.disableLighting = true;
    zLabelMaterial.backFaceCulling = false;
    zLabelPlane.material = zLabelMaterial;
  }

  /**
   * Set up Babylon 2D UI
   */
  private setupUI(): void {
    if (!this.engine || !this.scene) {
      return;
    }

    const advancedTexture = AdvancedDynamicTexture.CreateFullscreenUI('UI');
    
    // Recompute button
    const recomputeButton = Button.CreateSimpleButton('recomputeButton', 'Recompute Wave Collapse');
    recomputeButton.width = '200px';
    recomputeButton.height = '40px';
    recomputeButton.color = 'white';
    recomputeButton.background = 'green';
    recomputeButton.verticalAlignment = Control.VERTICAL_ALIGNMENT_TOP;
    recomputeButton.horizontalAlignment = Control.HORIZONTAL_ALIGNMENT_RIGHT;
    recomputeButton.top = '1%';
    recomputeButton.left = '-220px';
    recomputeButton.onPointerClickObservable.add(() => {
      this.renderGrid();
    });
    advancedTexture.addControl(recomputeButton);
    
    // Fullscreen button
    const fullscreenButton = Button.CreateSimpleButton('fullscreenButton', 'Fullscreen');
    fullscreenButton.width = '150px';
    fullscreenButton.height = '40px';
    fullscreenButton.color = 'white';
    fullscreenButton.background = 'blue';
    fullscreenButton.verticalAlignment = Control.VERTICAL_ALIGNMENT_TOP;
    fullscreenButton.horizontalAlignment = Control.HORIZONTAL_ALIGNMENT_RIGHT;
    fullscreenButton.top = '1%';
    fullscreenButton.left = '-10px';
    fullscreenButton.onPointerClickObservable.add(() => {
      if (this.engine) {
        this.engine.enterFullscreen(false);
      }
    });
    advancedTexture.addControl(fullscreenButton);
    
    // Exit fullscreen button
    const exitFullscreenButton = Button.CreateSimpleButton('exitFullscreenButton', 'Exit Fullscreen');
    exitFullscreenButton.width = '150px';
    exitFullscreenButton.height = '40px';
    exitFullscreenButton.color = 'white';
    exitFullscreenButton.background = 'red';
    exitFullscreenButton.verticalAlignment = Control.VERTICAL_ALIGNMENT_TOP;
    exitFullscreenButton.horizontalAlignment = Control.HORIZONTAL_ALIGNMENT_RIGHT;
    exitFullscreenButton.top = '1%';
    exitFullscreenButton.left = '-10px';
    exitFullscreenButton.isVisible = false;
    exitFullscreenButton.onPointerClickObservable.add(() => {
      if (this.engine) {
        this.engine.exitFullscreen();
      }
    });
    advancedTexture.addControl(exitFullscreenButton);
    
    // Handle fullscreen changes
    const handleFullscreenChange = (): void => {
      if (this.engine) {
        const isFullscreen = this.engine.isFullscreen;
        fullscreenButton.isVisible = !isFullscreen;
        exitFullscreenButton.isVisible = isFullscreen;
      }
    };
    
    document.addEventListener('fullscreenchange', handleFullscreenChange);
    document.addEventListener('webkitfullscreenchange', handleFullscreenChange);
    document.addEventListener('mozfullscreenchange', handleFullscreenChange);
    document.addEventListener('MSFullscreenChange', handleFullscreenChange);
  }

  /**
   * Render the WFC grid
   */
  renderGrid(constraints?: LayoutConstraints): void {
    const baseMeshForCleanup = this.baseMeshes.get('base');
    if (baseMeshForCleanup) {
      baseMeshForCleanup.thinInstanceCount = 0;
    }
    
    const wasmModule = this.wasmManager.getModule();
    if (!wasmModule) {
      return;
    }
    
    const constraintsToUse = constraints ?? getDefaultConstraints();
    
    // If using chunk-based rendering, we need to generate pre-constraints for all chunks
    if (this.worldMap) {
      const enabledChunks = this.worldMap.getEnabledChunks();
      
      if (enabledChunks.length > 0) {
        // Collect all hex coordinates from all enabled chunks
        const allHexCoords = new Set<string>();
        for (const chunk of enabledChunks) {
          const chunkGrid = chunk.getGrid();
          for (const chunkTile of chunkGrid) {
            allHexCoords.add(`${chunkTile.hex.q},${chunkTile.hex.r}`);
          }
        }
        
        // Find the bounding box to determine the center and rings needed
        let minQ = Number.POSITIVE_INFINITY;
        let maxQ = Number.NEGATIVE_INFINITY;
        let minR = Number.POSITIVE_INFINITY;
        let maxR = Number.NEGATIVE_INFINITY;
        
        for (const hexKey of allHexCoords) {
          const parts = hexKey.split(',');
          if (parts.length === 2) {
            const q = Number.parseInt(parts[0] ?? '0', 10);
            const r = Number.parseInt(parts[1] ?? '0', 10);
            if (!Number.isNaN(q) && !Number.isNaN(r)) {
              minQ = Math.min(minQ, q);
              maxQ = Math.max(maxQ, q);
              minR = Math.min(minR, r);
              maxR = Math.max(maxR, r);
            }
          }
        }
        
        // Calculate required rings to cover all chunks from origin (0, 0)
        // Since constraintsToPreConstraints is hardcoded to center at (0, 0),
        // we need to ensure we generate enough rings from (0, 0) to cover all tiles
        let maxDistanceFromOrigin = 0;
        for (const hexKey of allHexCoords) {
          const parts = hexKey.split(',');
          if (parts.length === 2) {
            const q = Number.parseInt(parts[0] ?? '0', 10);
            const r = Number.parseInt(parts[1] ?? '0', 10);
            if (!Number.isNaN(q) && !Number.isNaN(r)) {
              const distance = HexUtils.HEX_UTILS.distance(0, 0, q, r);
              maxDistanceFromOrigin = Math.max(maxDistanceFromOrigin, distance);
            }
          }
        }
        const requiredRings = Math.max(maxDistanceFromOrigin, this.currentRings);
        
        if (this.logFn) {
          this.log(`Total hex coordinates in chunks: ${allHexCoords.size}`, 'info');
          this.log(`Max distance from origin: ${maxDistanceFromOrigin}, required rings: ${requiredRings}`, 'info');
        }
        
        // Generate pre-constraints for the expanded grid
        if (this.generatePreConstraintsFn) {
          wasmModule.clear_pre_constraints();
          // Temporarily override rings to cover all chunks
          const originalRings = this.currentRings;
          this.currentRings = requiredRings;
          
          // Generate constraints with expanded area
          const expandedConstraints: LayoutConstraints = {
            ...constraintsToUse,
            rings: requiredRings,
          };
          
          const preConstraints = this.generatePreConstraintsFn(expandedConstraints);
          
          if (this.logFn) {
            this.log(`Generated ${preConstraints.length} pre-constraints`, 'info');
          }
          
          // Filter to only include hexes that are in our chunks
          let setCount = 0;
          for (const preConstraint of preConstraints) {
            const hexKey = `${preConstraint.q},${preConstraint.r}`;
            if (allHexCoords.has(hexKey)) {
              const tileNum = tileTypeToNumber(preConstraint.tileType);
              wasmModule.set_pre_constraint(preConstraint.q, preConstraint.r, tileNum);
              setCount++;
            }
          }
          
          if (this.logFn) {
            this.log(`Set ${setCount} pre-constraints for chunk tiles`, 'info');
          }
          
          // Restore original rings
          this.currentRings = originalRings;
        }
      }
    } else {
      // Original single-grid pre-constraint generation
      if (!constraints && this.generatePreConstraintsFn) {
        wasmModule.clear_pre_constraints();
        const preConstraints = this.generatePreConstraintsFn(constraintsToUse);
        for (const preConstraint of preConstraints) {
          const tileNum = tileTypeToNumber(preConstraint.tileType);
          wasmModule.set_pre_constraint(preConstraint.q, preConstraint.r, tileNum);
        }
      } else if (constraints && this.generatePreConstraintsFn) {
        // If constraints are provided, still generate pre-constraints
        wasmModule.clear_pre_constraints();
        const preConstraints = this.generatePreConstraintsFn(constraints);
        for (const preConstraint of preConstraints) {
          const tileNum = tileTypeToNumber(preConstraint.tileType);
          wasmModule.set_pre_constraint(preConstraint.q, preConstraint.r, tileNum);
        }
      }
    }
    
    wasmModule.generate_layout();
    
    // Create instances for each hex tile
    const hexSize = TILE_CONFIG.hexSize;
    const hexHeight = TILE_CONFIG.hexHeight;
    
    const baseMesh = this.baseMeshes.get('base');
    if (!baseMesh) {
      this.log('Base mesh not found for rendering', 'error');
      return;
    }
    
    // Prepare data for thin instances
    const validHexes: Array<{ hex: { q: number; r: number }; tileType: TileType; worldPos: Vector3 }> = [];
    
    // If worldMap is available, use chunk-based rendering
    if (this.worldMap) {
      const enabledChunks = this.worldMap.getEnabledChunks();
      
      if (this.logFn) {
        this.log(`Rendering ${enabledChunks.length} enabled chunks`, 'info');
      }
      
      // Collect tiles from all enabled chunks
      // Use a Set to deduplicate tiles at the same hex coordinate (chunks overlap at boundaries)
      const tileMap = new Map<string, { hex: { q: number; r: number }; tileType: TileType; worldPos: Vector3 }>();
      let totalTilesChecked = 0;
      let totalTilesFound = 0;
      let duplicateTiles = 0;
      
      for (const chunk of enabledChunks) {
        const chunkGrid = chunk.getGrid();
        const chunkPos = chunk.getPositionHex();
        let chunkTilesFound = 0;
        
        for (const chunkTile of chunkGrid) {
          totalTilesChecked++;
          // Query WASM for tile type at this hex coordinate
          const tileNum = wasmModule.get_tile_at(chunkTile.hex.q, chunkTile.hex.r);
          const tileType = tileTypeFromNumber(tileNum);
          
          if (!tileType) {
            continue;
          }
          
          chunkTilesFound++;
          totalTilesFound++;
          
          // Convert axial to world position
          const worldPos = HexUtils.HEX_UTILS.hexToWorld(chunkTile.hex.q, chunkTile.hex.r, hexSize);
          // Use absolute position (no centering needed for chunk-based rendering)
          const absolutePos = new Vector3(
            worldPos.x,
            hexHeight / 2.0,
            worldPos.z
          );
          
          // Use hex coordinate as key to deduplicate overlapping tiles from adjacent chunks
          const hexKey = `${chunkTile.hex.q},${chunkTile.hex.r}`;
          if (tileMap.has(hexKey)) {
            duplicateTiles++;
          }
          tileMap.set(hexKey, { hex: chunkTile.hex, tileType, worldPos: absolutePos });
        }
        
        if (this.logFn) {
          this.log(`Chunk at (${chunkPos.q}, ${chunkPos.r}): checked ${chunkGrid.length} tiles, found ${chunkTilesFound}`, 'info');
        }
      }
      
      // Convert map to array for rendering, sorted by hex coordinate for consistent ordering
      const sortedTiles = Array.from(tileMap.entries()).sort((a, b) => {
        const [q1, r1] = a[0].split(',').map((v) => Number.parseInt(v, 10));
        const [q2, r2] = b[0].split(',').map((v) => Number.parseInt(v, 10));
        if (q1 !== q2) {
          return q1 - q2;
        }
        return r1 - r2;
      });
      
      for (const [, tile] of sortedTiles) {
        validHexes.push(tile);
      }
      
      // Detect gaps between chunks
      // A gap is a hex position that is between chunk boundaries but not covered by any chunk
      let gapsFound = 0;
      const gapPositions: Array<{ q: number; r: number }> = [];
      
      if (enabledChunks.length > 0) {
        // Get rings from first chunk (all chunks should have same rings)
        const firstChunk = enabledChunks[0];
        if (firstChunk) {
          const firstChunkGrid = firstChunk.getGrid();
          // Calculate rings from grid size: 3*rings*(rings+1) + 1 = gridSize
          // Solve: 3*rings^2 + 3*rings + 1 - gridSize = 0
          // rings = (-3 + sqrt(9 + 12*(gridSize-1))) / 6
          const gridSize = firstChunkGrid.length;
          const rings = Math.round((-3 + Math.sqrt(9 + 12 * (gridSize - 1))) / 6);
          
          // Collect all hex positions that are actually covered
          const actualCoverage = new Set<string>();
          for (const chunk of enabledChunks) {
            const chunkGrid = chunk.getGrid();
            for (const tile of chunkGrid) {
              actualCoverage.add(`${tile.hex.q},${tile.hex.r}`);
            }
          }
          
          // Find the bounding area: all positions within (2*rings) distance of any chunk center
          // This is the area where chunks should be, and gaps would be visible
          const boundingArea = new Set<string>();
          for (const chunk of enabledChunks) {
            const chunkPos = chunk.getPositionHex();
            // Check all positions within distance (2*rings + rings) = 3*rings from chunk center
            // This covers the chunk itself plus the area where neighbors should be
            for (let checkDist = 0; checkDist <= 3 * rings; checkDist++) {
              const checkRing = HexUtils.HEX_UTILS.cubeRing(
                HexUtils.HEX_UTILS.axialToCube(chunkPos.q, chunkPos.r),
                checkDist
              );
              for (const cube of checkRing) {
                const axial = HexUtils.HEX_UTILS.cubeToAxial(cube);
                boundingArea.add(`${axial.q},${axial.r}`);
              }
            }
          }
          
          // Check for gaps: positions in bounding area that aren't covered
          for (const hexKey of boundingArea) {
            if (!actualCoverage.has(hexKey)) {
              const parts = hexKey.split(',');
              if (parts.length === 2) {
                const q = Number.parseInt(parts[0] ?? '0', 10);
                const r = Number.parseInt(parts[1] ?? '0', 10);
                if (!Number.isNaN(q) && !Number.isNaN(r)) {
                  // Check if this position is between chunks (within rings distance of at least one chunk center)
                  let isBetweenChunks = false;
                  for (const chunk of enabledChunks) {
                    const chunkPos = chunk.getPositionHex();
                    const dist = HexUtils.HEX_UTILS.distance(chunkPos.q, chunkPos.r, q, r);
                    // If it's beyond the chunk's own tiles (distance > rings) but close enough to be in the gap area
                    if (dist > rings && dist <= 2 * rings) {
                      isBetweenChunks = true;
                      break;
                    }
                  }
                  
                  if (isBetweenChunks) {
                    gapsFound++;
                    gapPositions.push({ q, r });
                  }
                }
              }
            }
          }
        }
      }
      
      if (this.logFn) {
        this.log(`Total tiles checked: ${totalTilesChecked}, tiles found in WASM: ${totalTilesFound}`, 'info');
        const duplicateLogType = duplicateTiles > 0 ? 'error' : 'info';
        this.log(`Duplicate tiles (overlapping chunks): ${duplicateTiles}, unique tiles: ${validHexes.length}`, duplicateLogType);
        
        if (gapsFound > 0) {
          this.log(`GAPS DETECTED: Found ${gapsFound} gaps between chunks!`, 'error');
          const sampleGaps = gapPositions.slice(0, 5);
          for (const gap of sampleGaps) {
            this.log(`  Gap at hex (${gap.q}, ${gap.r})`, 'error');
          }
          if (gapPositions.length > 5) {
            this.log(`  ... and ${gapPositions.length - 5} more gaps`, 'error');
          }
        } else {
          this.log('No gaps detected between chunks', 'info');
        }
        
        // Count tiles from each chunk that made it into the final render
        const chunkTileCounts = new Map<string, number>();
        for (const tile of validHexes) {
          // Find which chunk(s) this tile belongs to
          for (const chunk of enabledChunks) {
            const chunkPos = chunk.getPositionHex();
            const chunkGrid = chunk.getGrid();
            const belongsToChunk = chunkGrid.some((ct) => ct.hex.q === tile.hex.q && ct.hex.r === tile.hex.r);
            if (belongsToChunk) {
              const chunkKey = `${chunkPos.q},${chunkPos.r}`;
              chunkTileCounts.set(chunkKey, (chunkTileCounts.get(chunkKey) ?? 0) + 1);
            }
          }
        }
        
        this.log(`Tiles per chunk in final render:`, 'info');
        for (const [chunkKey, count] of chunkTileCounts.entries()) {
          this.log(`  Chunk ${chunkKey}: ${count} tiles`, 'info');
        }
        
        // Log sample positions from different chunks
        const sampleChunks = enabledChunks.slice(0, 3);
        for (const chunk of sampleChunks) {
          const chunkPos = chunk.getPositionHex();
          const chunkGrid = chunk.getGrid();
          if (chunkGrid.length > 0) {
            const firstTile = chunkGrid[0];
            if (firstTile) {
              const worldPos = HexUtils.HEX_UTILS.hexToWorld(firstTile.hex.q, firstTile.hex.r, hexSize);
              this.log(`Chunk (${chunkPos.q}, ${chunkPos.r}) first tile at hex (${firstTile.hex.q}, ${firstTile.hex.r}) -> world (${worldPos.x.toFixed(2)}, ${worldPos.z.toFixed(2)})`, 'info');
            }
          }
        }
        
        // Log some tiles from neighbor chunks to verify they're in the render list
        const neighborChunkTiles: Array<{ chunk: string; hex: string; world: string }> = [];
        for (const tile of validHexes) {
          // Check if this tile is from a neighbor chunk (not origin)
          for (const chunk of enabledChunks) {
            const chunkPos = chunk.getPositionHex();
            if (chunkPos.q === 0 && chunkPos.r === 0) {
              continue; // Skip origin chunk
            }
            const chunkGrid = chunk.getGrid();
            const belongsToChunk = chunkGrid.some((ct) => ct.hex.q === tile.hex.q && ct.hex.r === tile.hex.r);
            if (belongsToChunk) {
              neighborChunkTiles.push({
                chunk: `(${chunkPos.q}, ${chunkPos.r})`,
                hex: `(${tile.hex.q}, ${tile.hex.r})`,
                world: `(${tile.worldPos.x.toFixed(2)}, ${tile.worldPos.z.toFixed(2)})`,
              });
              if (neighborChunkTiles.length >= 5) {
                break;
              }
            }
          }
          if (neighborChunkTiles.length >= 5) {
            break;
          }
        }
        
        if (neighborChunkTiles.length > 0) {
          this.log(`Sample neighbor chunk tiles in render:`, 'info');
          for (const sample of neighborChunkTiles) {
            this.log(`  Chunk ${sample.chunk} tile at hex ${sample.hex} -> world ${sample.world}`, 'info');
          }
        } else {
          this.log('WARNING: No neighbor chunk tiles found in final render!', 'error');
        }
      }
    } else {
      // Fallback to original single-grid rendering
      const renderRings = this.currentRings;
      
      if (this.logFn) {
        this.log(`Rendering with rings: ${renderRings} (expected tiles: ${3 * renderRings * (renderRings + 1) + 1})`, 'info');
      }
      
      // Center at (0, 0) - hexagon centered at origin
      const renderCenterQ = 0;
      const renderCenterR = 0;
      
      // Generate hexagon grid
      const renderHexGrid = HexUtils.HEX_UTILS.generateHexGrid(renderRings, renderCenterQ, renderCenterR);
      
      const centerWorldPos = HexUtils.HEX_UTILS.hexToWorld(renderCenterQ, renderCenterR, hexSize);
      
      for (const hex of renderHexGrid) {
        // Query WASM for tile type at this hex coordinate
        const tileNum = wasmModule.get_tile_at(hex.q, hex.r);
        const tileType = tileTypeFromNumber(tileNum);
        
        if (!tileType) {
          continue;
        }
        
        // Convert axial to world position
        const worldPos = HexUtils.HEX_UTILS.hexToWorld(hex.q, hex.r, hexSize);
        // Center the grid by subtracting center hex's position
        const centeredPos = new Vector3(
          worldPos.x - centerWorldPos.x,
          hexHeight / 2.0,
          worldPos.z - centerWorldPos.z
        );
        
        validHexes.push({ hex, tileType, worldPos: centeredPos });
      }
    }
    
    const numInstances = validHexes.length;
    
    if (numInstances === 0) {
      if (this.logFn) {
        this.log('No valid hexes to render', 'warning');
      }
      return;
    }
    
    if (this.logFn) {
      this.log(`Creating ${numInstances} thin instances`, 'info');
      
      // Log sample positions from different areas
      if (validHexes.length > 0) {
        const firstTile = validHexes[0];
        const middleTile = validHexes[Math.floor(validHexes.length / 2)];
        const lastTile = validHexes[validHexes.length - 1];
        
        if (firstTile && middleTile && lastTile) {
          this.log(`Sample positions - First: (${firstTile.worldPos.x.toFixed(2)}, ${firstTile.worldPos.z.toFixed(2)}), Middle: (${middleTile.worldPos.x.toFixed(2)}, ${middleTile.worldPos.z.toFixed(2)}), Last: (${lastTile.worldPos.x.toFixed(2)}, ${lastTile.worldPos.z.toFixed(2)})`, 'info');
        }
        
        // Find min/max positions
        let minX = Number.POSITIVE_INFINITY;
        let maxX = Number.NEGATIVE_INFINITY;
        let minZ = Number.POSITIVE_INFINITY;
        let maxZ = Number.NEGATIVE_INFINITY;
        
        for (const tile of validHexes) {
          minX = Math.min(minX, tile.worldPos.x);
          maxX = Math.max(maxX, tile.worldPos.x);
          minZ = Math.min(minZ, tile.worldPos.z);
          maxZ = Math.max(maxZ, tile.worldPos.z);
        }
        
        this.log(`World bounds - X: [${minX.toFixed(2)}, ${maxX.toFixed(2)}], Z: [${minZ.toFixed(2)}, ${maxZ.toFixed(2)}]`, 'info');
      }
    }
    
    const matrices = new Float32Array(numInstances * 16);
    const bufferColors = new Float32Array(numInstances * 4);
    const baseMeshScaling = baseMesh.scaling.clone();
    
    for (let i = 0; i < numInstances; i++) {
      const { tileType, worldPos } = validHexes[i];
      const translation = new Vector3(worldPos.x, worldPos.y, worldPos.z);
      const scaling = baseMeshScaling.clone();
      const rotation = Quaternion.Identity();
      const matrix = Matrix.Compose(scaling, rotation, translation);
      matrix.copyToArray(matrices, i * 16);
      
      const color = getTileColor(tileType);
      bufferColors[i * 4] = color.r;
      bufferColors[i * 4 + 1] = color.g;
      bufferColors[i * 4 + 2] = color.b;
      bufferColors[i * 4 + 3] = 1.0;
    }
    
    // Verify sample matrices have correct positions
    if (this.logFn && numInstances > 0) {
      // Check first, middle, and last matrices
      const indicesToCheck = [0, Math.floor(numInstances / 2), numInstances - 1];
      for (const idx of indicesToCheck) {
        if (idx < numInstances) {
          const matrixStart = idx * 16;
          // Matrix translation is at indices 12, 13, 14 (m[12], m[13], m[14])
          const matrixX = matrices[matrixStart + 12];
          const matrixY = matrices[matrixStart + 13];
          const matrixZ = matrices[matrixStart + 14];
          const expectedTile = validHexes[idx];
          if (expectedTile) {
            this.log(`Matrix ${idx}: translation (${matrixX.toFixed(2)}, ${matrixY.toFixed(2)}, ${matrixZ.toFixed(2)}), expected (${expectedTile.worldPos.x.toFixed(2)}, ${expectedTile.worldPos.y.toFixed(2)}, ${expectedTile.worldPos.z.toFixed(2)})`, 'info');
          }
        }
      }
    }
    
    baseMesh.thinInstanceSetBuffer("matrix", matrices, 16);
    // Use "instanceColor" attribute name for thin instance colors (not "color")
    baseMesh.thinInstanceSetBuffer("instanceColor", bufferColors, 4);
    baseMesh.thinInstanceCount = numInstances;
    
    if (this.logFn) {
      this.log(`Rendered ${numInstances} instances (baseMesh.thinInstanceCount = ${baseMesh.thinInstanceCount})`, 'info');
    }
    
    const baseMaterial = this.materials.get('grass');
    if (baseMaterial) {
      baseMesh.material = baseMaterial;
    }
    
    baseMesh.isVisible = true;
  }

  /**
   * Reset camera to initial position
   */
  resetCamera(): void {
    if (!this.camera || !this.scene) {
      return;
    }

    const gridCenter = new Vector3(
      CAMERA_CONFIG.gridCenter.x,
      CAMERA_CONFIG.gridCenter.y,
      CAMERA_CONFIG.gridCenter.z
    );
    this.camera.alpha = CAMERA_CONFIG.initialAlpha;
    this.camera.beta = CAMERA_CONFIG.initialBeta;
    this.camera.radius = CAMERA_CONFIG.initialRadius;
    this.camera.setTarget(gridCenter);
  }

  /**
   * Get the camera
   */
  getCamera(): ArcRotateCamera | null {
    return this.camera;
  }

  /**
   * Update test mode and recreate axis visualizer if needed
   */
  setTestMode(isTestMode: boolean): void {
    if (this.isTestMode === isTestMode) {
      return;
    }

    this.isTestMode = isTestMode;

    if (!this.scene) {
      return;
    }

    // Remove existing axis visualizer if any
    const xAxis = this.scene.getMeshByName('xAxis');
    const yAxis = this.scene.getMeshByName('yAxis');
    const zAxis = this.scene.getMeshByName('zAxis');
    const xLabelPlane = this.scene.getMeshByName('xLabelPlane');
    const yLabelPlane = this.scene.getMeshByName('yLabelPlane');
    const zLabelPlane = this.scene.getMeshByName('zLabelPlane');

    if (xAxis) {
      xAxis.dispose();
    }
    if (yAxis) {
      yAxis.dispose();
    }
    if (zAxis) {
      zAxis.dispose();
    }
    if (xLabelPlane) {
      xLabelPlane.dispose();
    }
    if (yLabelPlane) {
      yLabelPlane.dispose();
    }
    if (zLabelPlane) {
      zLabelPlane.dispose();
    }

    // Create axis visualizer if in test mode
    if (this.isTestMode) {
      this.createAxisVisualizer();
    }
  }

  /**
   * Set the background color of the scene
   */
  setBackgroundColor(hexColor: string): void {
    if (!this.scene) {
      return;
    }

    // Parse hex color to RGB
    const hex = hexColor.replace('#', '');
    const r = Number.parseInt(hex.substring(0, 2), 16) / 255;
    const g = Number.parseInt(hex.substring(2, 4), 16) / 255;
    const b = Number.parseInt(hex.substring(4, 6), 16) / 255;

    // Set clear color (RGBA, alpha = 1.0 for opaque)
    this.scene.clearColor = new Color4(r, g, b, 1.0);
  }

  /**
   * Dispose of the canvas manager and clean up resources
   */
  dispose(): void {
    if (this.scene) {
      this.scene.dispose();
      this.scene = null;
    }

    if (this.engine) {
      this.engine.dispose();
      this.engine = null;
    }

    this.camera = null;
    this.baseMeshes.clear();
    this.materials.clear();
    this.worldMap = null;
  }
}

