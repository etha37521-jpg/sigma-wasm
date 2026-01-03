/**
 * Chunk Management Module
 * 
 * Handles chunk-based tile management for hexagonal grid layouts.
 * Each chunk contains tiles arranged in rings around a central position.
 */

import type { TileType } from '../../types';
import * as HexUtils from './hexUtils';

/**
 * Tile entry in a chunk's grid
 */
export interface ChunkTile {
  hex: HexUtils.HexCoord;
  tileType: TileType | null;
}

/**
 * Chunk class representing a collection of tiles in rings around a central position
 */
export class Chunk {
  private grid: Array<ChunkTile>;
  private positionHex: HexUtils.HexCoord;
  private positionCartesian: { x: number; z: number };
  private enabled: boolean;
  private neighbors: Array<HexUtils.HexCoord>;

  /**
   * Create a new chunk
   * @param positionHex - Central cell position in hex space (q, r)
   * @param rings - Number of rings around the center
   * @param hexSize - Size of hexagon for coordinate conversion
   */
  constructor(
    positionHex: HexUtils.HexCoord,
    rings: number,
    hexSize: number
  ) {
    this.positionHex = positionHex;
    this.enabled = true;
    
    // Convert hex position to Cartesian for absolute positioning
    const worldPos = HexUtils.HEX_UTILS.hexToWorld(positionHex.q, positionHex.r, hexSize);
    this.positionCartesian = { x: worldPos.x, z: worldPos.z };
    
    // Generate grid in rings around the central position
    const hexGrid = HexUtils.HEX_UTILS.generateHexGrid(rings, positionHex.q, positionHex.r);
    
    // Validate that all generated tiles are within the ring count
    const validatedGrid: Array<ChunkTile> = [];
    for (const hex of hexGrid) {
      const distance = HexUtils.HEX_UTILS.distance(positionHex.q, positionHex.r, hex.q, hex.r);
      if (distance <= rings) {
        validatedGrid.push({
          hex,
          tileType: null,
        });
      }
    }
    
    this.grid = validatedGrid;
    
    // Compute neighbor chunk positions
    // For chunks to be packed without gaps or overlap:
    // - A chunk with 'rings' rings contains tiles from distance 0 to 'rings' from its center
    // - Neighbor chunk centers are calculated using offset vector (rings, rings+1) rotated 6 times
    // - This ensures chunks touch at their boundaries: origin's outer boundary (distance 'rings') 
    //   is adjacent to neighbor's outer boundary (distance 'rings' from neighbor center)
    // - For rings=0: uses offset (1, 0) rotated 6 times (distance 1 neighbors)
    // - For rings=1: uses offset (1, 2) rotated 6 times (distance 3 neighbors)
    // - For rings=2: uses offset (2, 3) rotated 6 times (distance 5 neighbors)
    this.neighbors = this.calculateChunkNeighbors(positionHex, rings);
  }

  /**
   * Get the chunk's grid of tiles
   */
  getGrid(): Array<ChunkTile> {
    return this.grid;
  }

  /**
   * Get the chunk's central position in hex space
   */
  getPositionHex(): HexUtils.HexCoord {
    return this.positionHex;
  }

  /**
   * Get the chunk's absolute position in Cartesian space
   */
  getPositionCartesian(): { x: number; z: number } {
    return this.positionCartesian;
  }

  /**
   * Get whether the chunk's tiles are enabled
   */
  getEnabled(): boolean {
    return this.enabled;
  }

  /**
   * Get the chunk's neighbor positions
   */
  getNeighbors(): Array<HexUtils.HexCoord> {
    return this.neighbors;
  }

  /**
   * Calculate chunk neighbor positions using offset vector rotation
   * Returns exactly 6 neighbor hex coordinates, one in each of the 6 directions
   * 
   * Uses the offset vector (rings, rings+1) for rings>0, or (1, 0) for rings=0, and rotates
   * it 60 degrees counter-clockwise 6 times. This ensures chunks are packed without gaps - 
   * each direction has exactly one neighbor. The outer boundaries of adjacent chunks touch.
   * 
   * The rotation formula in axial coordinates: (q, r) -> (-r, q+r)
   * This produces neighbors at distance 2*rings+1 (or distance 1 for rings=0).
   * 
   * @param center - Center hex coordinate
   * @param rings - Number of rings in the chunk
   * @returns Array of exactly 6 neighbor hex coordinates
   */
  private calculateChunkNeighbors(center: HexUtils.HexCoord, rings: number): Array<HexUtils.HexCoord> {
    const neighbors: Array<HexUtils.HexCoord> = [];
    
    // Base offset vector: (rings, rings+1) for rings>0, or (1, 0) for rings=0
    let offsetQ: number;
    let offsetR: number;
    if (rings === 0) {
      offsetQ = 1;
      offsetR = 0;
    } else {
      offsetQ = rings;
      offsetR = rings + 1;
    }
    
    // Rotate the offset vector 60 degrees counter-clockwise 6 times
    // Rotation formula in axial coordinates: (q, r) -> (-r, q+r)
    let currentQ = offsetQ;
    let currentR = offsetR;
    
    for (let i = 0; i < 6; i++) {
      // Add the current offset to the center
      neighbors.push({ q: center.q + currentQ, r: center.r + currentR });
      
      // Rotate 60 degrees counter-clockwise: (q, r) -> (-r, q+r)
      const nextQ = -currentR;
      const nextR = currentQ + currentR;
      currentQ = nextQ;
      currentR = nextR;
    }

    return neighbors;
  }

  /**
   * Set the enabled state of the chunk's tiles
   * Updates the enabled property accordingly
   * 
   * Note: This method updates the chunk's enabled state.
   * Actual mesh visibility is controlled during rendering.
   */
  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
  }

  /**
   * Set tile type for a specific hex coordinate in this chunk
   */
  setTileType(hex: HexUtils.HexCoord, tileType: TileType | null): void {
    const tile = this.grid.find((t) => t.hex.q === hex.q && t.hex.r === hex.r);
    if (tile) {
      tile.tileType = tileType;
    }
  }

  /**
   * Get tile type for a specific hex coordinate in this chunk
   */
  getTileType(hex: HexUtils.HexCoord): TileType | null {
    const tile = this.grid.find((t) => t.hex.q === hex.q && t.hex.r === hex.r);
    return tile ? tile.tileType : null;
  }
}

/**
 * WorldMap class managing all chunks in the world
 */
export class WorldMap {
  private chunks: globalThis.Map<string, Chunk>;

  constructor() {
    this.chunks = new globalThis.Map<string, Chunk>();
  }

  /**
   * Get a chunk key string from hex coordinates
   */
  private getChunkKey(positionHex: HexUtils.HexCoord): string {
    return `${positionHex.q},${positionHex.r}`;
  }

  /**
   * Get a chunk by its position in hex space
   */
  getChunk(positionHex: HexUtils.HexCoord): Chunk | undefined {
    const key = this.getChunkKey(positionHex);
    return this.chunks.get(key);
  }

  /**
   * Create a new chunk at the specified position
   * @param positionHex - Central cell position in hex space (q, r)
   * @param rings - Number of rings around the center
   * @param hexSize - Size of hexagon for coordinate conversion
   * @returns The created chunk
   */
  createChunk(
    positionHex: HexUtils.HexCoord,
    rings: number,
    hexSize: number
  ): Chunk {
    const key = this.getChunkKey(positionHex);
    
    // Check if chunk already exists
    const existing = this.chunks.get(key);
    if (existing) {
      return existing;
    }
    
    // Create new chunk
    const chunk = new Chunk(positionHex, rings, hexSize);
    this.chunks.set(key, chunk);
    
    return chunk;
  }

  /**
   * Get all instantiated chunks
   */
  getAllChunks(): Array<Chunk> {
    return Array.from(this.chunks.values());
  }

  /**
   * Get all enabled chunks
   */
  getEnabledChunks(): Array<Chunk> {
    return this.getAllChunks().filter((chunk) => chunk.getEnabled());
  }

  /**
   * Check if a chunk exists at the specified position
   */
  hasChunk(positionHex: HexUtils.HexCoord): boolean {
    const key = this.getChunkKey(positionHex);
    return this.chunks.has(key);
  }

  /**
   * Get the number of chunks
   */
  getChunkCount(): number {
    return this.chunks.size;
  }
}

