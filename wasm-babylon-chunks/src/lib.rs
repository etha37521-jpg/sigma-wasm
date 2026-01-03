use wasm_bindgen::prelude::*;
use std::sync::{LazyLock, Mutex};
use std::collections::{HashMap, HashSet, BinaryHeap};
use std::cmp::Ordering;

/// Tile type enumeration for 5 simple tile types
/// 
/// **Learning Point**: Simplified tile types for hex grid layout generation.
/// Each tile type represents a terrain or structure type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TileType {
    Grass = 0,
    Building = 1,
    Road = 2,
    Forest = 3,
    Water = 4,
}

/// State structure using hash map for efficient sparse grid storage
/// 
/// **Learning Point**: Uses HashMap<(i32, i32), TileType> for O(1) lookups and
/// no size limitations. Keys are (q, r) hex coordinates.
struct WfcState {
    grid: HashMap<(i32, i32), TileType>,
    pre_constraints: HashMap<(i32, i32), TileType>,
}

impl WfcState {
    fn new() -> Self {
        WfcState {
            grid: HashMap::new(),
            pre_constraints: HashMap::new(),
        }
    }
    
    fn clear(&mut self) {
        self.grid.clear();
        // DO NOT clear pre_constraints - they must persist
    }
    
    /// Set a pre-constraint at a specific hex position (q, r)
    /// Returns true if the constraint was set successfully
    fn set_pre_constraint(&mut self, q: i32, r: i32, tile_type: TileType) -> bool {
        self.pre_constraints.insert((q, r), tile_type);
        true
    }
    
    /// Clear all pre-constraints
    fn clear_pre_constraints(&mut self) {
        self.pre_constraints.clear();
    }
    
    /// Get tile at hex coordinate (q, r)
    fn get_tile(&self, q: i32, r: i32) -> Option<TileType> {
        self.grid.get(&(q, r)).copied()
    }
}

/// Hex coordinate structure for Voronoi generation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct HexCoord {
    q: i32,
    r: i32,
}

/// Seed point for Voronoi region generation
#[derive(Clone, Copy, Debug)]
struct VoronoiSeed {
    q: i32,
    r: i32,
    tile_type: TileType,
}

/// Calculate hex distance between two hex coordinates (cube distance)
/// Uses axial coordinates converted to cube coordinates
/// Formula: (|dq| + |dr| + |ds|) / 2 where s = -q - r
/// This matches the Python example: (abs(q1-q2) + abs(r1-r2) + abs(s1-s2)) // 2
fn hex_distance(q1: i32, r1: i32, q2: i32, r2: i32) -> i32 {
    let s1 = -q1 - r1;
    let s2 = -q2 - r2;
    ((q1 - q2).abs() + (r1 - r2).abs() + (s1 - s2).abs()) / 2
}

/// A* node for pathfinding with parent pointer for path reconstruction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AStarNode {
    q: i32,
    r: i32,
    g: i32,
    h: i32,
    f: i32,
    parent_q: i32,
    parent_r: i32,
}

impl AStarNode {
    fn new(q: i32, r: i32, g: i32, h: i32, parent_q: i32, parent_r: i32) -> Self {
        AStarNode {
            q,
            r,
            g,
            h,
            f: g + h,
            parent_q,
            parent_r,
        }
    }
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap (lowest f score first)
        other.f.cmp(&self.f)
            .then_with(|| other.h.cmp(&self.h))
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Get all 6 hex neighbors of a coordinate (axial)
fn get_hex_neighbors(q: i32, r: i32) -> Vec<(i32, i32)> {
    vec![
        (q + 1, r),
        (q - 1, r),
        (q, r + 1),
        (q, r - 1),
        (q + 1, r - 1),
        (q - 1, r + 1),
    ]
}

/// Convert axial coordinates to cube coordinates
/// Cube coordinates: (q, r, s) where q + r + s = 0
fn axial_to_cube(q: i32, r: i32) -> CubeCoord {
    CubeCoord {
        q,
        r,
        s: -q - r,
    }
}

/// Calculate cube distance between two cube coordinates
/// Formula: max(|dq|, |dr|, |ds|)
/// This matches TypeScript HEX_UTILS.cubeDistance
fn cube_distance(a: CubeCoord, b: CubeCoord) -> i32 {
    (a.q - b.q).abs().max((a.r - b.r).abs()).max((a.s - b.s).abs())
}

/// Parse valid terrain JSON string into HashSet
/// Format: [{"q":0,"r":0},{"q":1,"r":0},...]
/// Returns empty HashSet if parsing fails
fn parse_valid_terrain_json(valid_terrain_json: &str) -> HashSet<(i32, i32)> {
    let mut valid_terrain = HashSet::new();
    
    let trimmed = valid_terrain_json.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return valid_terrain;
    }
    
    // Simple JSON parsing: find all {"q":X,"r":Y} patterns
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() {
        // Look for opening brace
        if chars[i] == '{' {
            let mut q_value: Option<i32> = None;
            let mut r_value: Option<i32> = None;
            
            i += 1;
            while i < chars.len() && chars[i] != '}' {
                // Look for "q" or "r" followed by colon and number
                if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'q' && chars[i + 2] == '"' {
                    i += 3;
                    // Skip colon and whitespace
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    // Parse number
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            q_value = Some(num);
                        }
                    }
                } else if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'r' && chars[i + 2] == '"' {
                    i += 3;
                    // Skip colon and whitespace
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    // Parse number
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            r_value = Some(num);
                        }
                    }
                } else {
                    i += 1;
                }
            }
            
            if let (Some(q), Some(r)) = (q_value, r_value) {
                valid_terrain.insert((q, r));
            }
        }
        i += 1;
    }
    
    valid_terrain
}

/// Hex A* pathfinding between two road tiles
/// Returns path length, or -1 if unreachable
/// Only considers road tiles as valid path nodes
/// 
/// Algorithm matches Python example:
/// - Uses f_cost = g_cost + h_cost for priority
/// - g_cost is path cost from start (uniform cost of 1 per step)
/// - h_cost is hex distance heuristic
/// - Explores nodes with lowest f_cost first
fn hex_astar_path(
    start_q: i32,
    start_r: i32,
    goal_q: i32,
    goal_r: i32,
    roads: &HashSet<(i32, i32)>,
) -> i32 {
    // Check if start and goal are roads
    if !roads.contains(&(start_q, start_r)) || !roads.contains(&(goal_q, goal_r)) {
        return -1;
    }

    // If start equals goal, path length is 0
    if start_q == goal_q && start_r == goal_r {
        return 0;
    }

    // Calculate heuristic (hex distance) - now using correct formula
    let h_start = hex_distance(start_q, start_r, goal_q, goal_r);

    let mut open_set = BinaryHeap::new();
    let mut closed_set = HashSet::new();
    let mut g_scores: HashMap<(i32, i32), i32> = HashMap::new();

    open_set.push(AStarNode::new(start_q, start_r, 0, h_start, start_q, start_r));
    g_scores.insert((start_q, start_r), 0);

    while let Some(current) = open_set.pop() {
        let current_key = (current.q, current.r);

        // Skip if already processed (duplicate in open_set)
        if closed_set.contains(&current_key) {
            continue;
        }

        closed_set.insert(current_key);

        // Check if we reached the goal
        if current.q == goal_q && current.r == goal_r {
            return current.g;
        }

        // Explore neighbors - get all 6 hex neighbors
        let neighbors = get_hex_neighbors(current.q, current.r);
        for (nq, nr) in neighbors {
            let neighbor_key = (nq, nr);

            // Skip if not a road (obstacle check)
            if !roads.contains(&neighbor_key) {
                continue;
            }

            // Skip if already closed
            if closed_set.contains(&neighbor_key) {
                continue;
            }

            // Calculate tentative g score (uniform cost of 1 per step)
            let tentative_g = current.g + 1;

            // Check if this is a better path (matches Python: if neighbor not in g_cost or tentative_g < g_cost[neighbor])
            let current_g = g_scores.get(&neighbor_key).copied().unwrap_or(i32::MAX);
            if tentative_g < current_g {
                // This path to neighbor is better - record it
                g_scores.insert(neighbor_key, tentative_g);
                let h = hex_distance(nq, nr, goal_q, goal_r);
                open_set.push(AStarNode::new(nq, nr, tentative_g, h, current.q, current.r));
            }
        }
    }

    // No path found
    -1
}

/// Hex A* pathfinding that returns full path
/// Matches TypeScript hexAStar algorithm exactly:
/// - Uses cube coordinates for distance calculation (cube_distance)
/// - Maintains open set as BinaryHeap (min-heap by f score, then h score)
/// - Maintains closed set as HashSet
/// - Maintains g_scores as HashMap
/// - Stores parent pointers for path reconstruction
/// 
/// @param start_q - Start q coordinate (axial)
/// @param start_r - Start r coordinate (axial)
/// @param goal_q - Goal q coordinate (axial)
/// @param goal_r - Goal r coordinate (axial)
/// @param valid_terrain_json - JSON string with array of valid terrain coordinates: [{"q":0,"r":0},...]
/// @returns JSON string with path array [{"q":0,"r":0},...] or "null" if no path found
#[wasm_bindgen]
pub fn hex_astar(
    start_q: i32,
    start_r: i32,
    goal_q: i32,
    goal_r: i32,
    valid_terrain_json: String,
) -> String {
    // Parse valid terrain from JSON
    let valid_terrain = parse_valid_terrain_json(&valid_terrain_json);
    
    // Check if start and goal are in valid terrain
    if !valid_terrain.contains(&(start_q, start_r)) || !valid_terrain.contains(&(goal_q, goal_r)) {
        return "null".to_string();
    }
    
    // If start equals goal, return path with single node
    if start_q == goal_q && start_r == goal_r {
        return format!(r#"[{{"q":{},"r":{}}}]"#, start_q, start_r);
    }
    
    // Convert goal to cube for distance calculation (matches TypeScript)
    let goal_cube = axial_to_cube(goal_q, goal_r);
    
    // Calculate heuristic function (cube distance)
    let heuristic = |q: i32, r: i32| -> i32 {
        let cube = axial_to_cube(q, r);
        cube_distance(cube, goal_cube)
    };
    
    // Initialize A* data structures
    let h_start = heuristic(start_q, start_r);
    let mut open_set = BinaryHeap::new();
    let mut closed_set = HashSet::new();
    let mut g_scores: HashMap<(i32, i32), i32> = HashMap::new();
    let mut parents: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    
    // Start node (parent is itself to mark as root)
    open_set.push(AStarNode::new(start_q, start_r, 0, h_start, start_q, start_r));
    g_scores.insert((start_q, start_r), 0);
    
    while let Some(current) = open_set.pop() {
        let current_key = (current.q, current.r);
        
        // Skip if already processed (duplicate in open_set)
        if closed_set.contains(&current_key) {
            continue;
        }
        
        closed_set.insert(current_key);
        
        // Check if we reached the goal
        if current.q == goal_q && current.r == goal_r {
            // Reconstruct path by following parent pointers
            let mut path: Vec<(i32, i32)> = Vec::new();
            let mut node_key = (goal_q, goal_r);
            
            // Follow parent pointers from goal to start
            loop {
                path.push(node_key);
                
                // Get parent for this node
                if let Some(parent_key) = parents.get(&node_key) {
                    // If parent is the start, add it and break
                    if parent_key.0 == start_q && parent_key.1 == start_r {
                        path.push((start_q, start_r));
                        break;
                    }
                    node_key = *parent_key;
                } else {
                    // No parent in map means we're at start (shouldn't happen in normal flow)
                    // But handle it just in case
                    if node_key.0 != start_q || node_key.1 != start_r {
                        path.push((start_q, start_r));
                    }
                    break;
                }
            }
            
            // Reverse path to get start-to-goal order
            path.reverse();
            
            // Build JSON string
            let mut json_parts = Vec::new();
            for (q, r) in path {
                json_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
            }
            
            return format!("[{}]", json_parts.join(","));
        }
        
        // Explore neighbors
        let neighbors = get_hex_neighbors(current.q, current.r);
        for (nq, nr) in neighbors {
            let neighbor_key = (nq, nr);
            
            // Skip if not in valid terrain
            if !valid_terrain.contains(&neighbor_key) {
                continue;
            }
            
            // Skip if already closed
            if closed_set.contains(&neighbor_key) {
                continue;
            }
            
            // Calculate tentative g score (uniform cost of 1 per step)
            let tentative_g = current.g + 1;
            
            // Check if this is a better path
            let current_g = g_scores.get(&neighbor_key).copied().unwrap_or(i32::MAX);
            if tentative_g < current_g {
                // This path to neighbor is better - record it
                g_scores.insert(neighbor_key, tentative_g);
                parents.insert(neighbor_key, (current.q, current.r));
                let h = heuristic(nq, nr);
                open_set.push(AStarNode::new(nq, nr, tentative_g, h, current.q, current.r));
            }
        }
    }
    
    // No path found
    "null".to_string()
}

/// Build a path between two road points using A* pathfinding
/// Returns array of intermediate hexes (excluding start, including end)
/// Matches TypeScript buildPathBetweenRoads function
/// 
/// @param start_q - Start q coordinate (axial)
/// @param start_r - Start r coordinate (axial)
/// @param end_q - End q coordinate (axial)
/// @param end_r - End r coordinate (axial)
/// @param valid_terrain_json - JSON string with array of valid terrain coordinates: [{"q":0,"r":0},...]
/// @returns JSON string with path array excluding start, including end, or "null" if no path found
#[wasm_bindgen]
pub fn build_path_between_roads(
    start_q: i32,
    start_r: i32,
    end_q: i32,
    end_r: i32,
    valid_terrain_json: String,
) -> String {
    // Call hex_astar to get full path
    let full_path_json = hex_astar(start_q, start_r, end_q, end_r, valid_terrain_json);
    
    // If no path, return null
    if full_path_json == "null" || full_path_json.is_empty() {
        return "null".to_string();
    }
    
    // Parse the path JSON
    // Simple parsing: extract all {"q":X,"r":Y} patterns and skip first one
    let trimmed = full_path_json.trim();
    if trimmed == "[]" || trimmed.len() < 3 {
        return "null".to_string();
    }
    
    // Find all coordinate pairs
    let mut coords: Vec<(i32, i32)> = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() {
        if chars[i] == '{' {
            let mut q_value: Option<i32> = None;
            let mut r_value: Option<i32> = None;
            
            i += 1;
            while i < chars.len() && chars[i] != '}' {
                if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'q' && chars[i + 2] == '"' {
                    i += 3;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            q_value = Some(num);
                        }
                    }
                } else if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'r' && chars[i + 2] == '"' {
                    i += 3;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            r_value = Some(num);
                        }
                    }
                } else {
                    i += 1;
                }
            }
            
            if let (Some(q), Some(r)) = (q_value, r_value) {
                coords.push((q, r));
            }
        }
        i += 1;
    }
    
    // If path has less than 2 nodes, return null
    if coords.len() < 2 {
        return "null".to_string();
    }
    
    // Return path excluding start (first element), including end (last element)
    let path_without_start = &coords[1..];
    
    // Build JSON string
    let mut json_parts = Vec::new();
    for (q, r) in path_without_start {
        json_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    
    format!("[{}]", json_parts.join(","))
}

/// Validate that all road tiles are reachable from each other using A* pathfinding
/// 
/// Uses transitive property: if all roads are reachable from one source road,
/// then all pairs have paths (by transitivity: A->B and B->C implies A->C).
/// 
/// @param roads_json - JSON string with array of road coordinates: [{"q":0,"r":0},{"q":1,"r":0},...]
/// @returns true if all roads are reachable from source, false otherwise
#[wasm_bindgen]
pub fn validate_road_connectivity(roads_json: String) -> bool {
    // Parse roads from JSON
    // Simple JSON parsing without serde to keep WASM size small
    let mut roads: Vec<(i32, i32)> = Vec::new();
    
    // Remove whitespace and brackets
    let trimmed = roads_json.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return true; // Empty roads is trivially connected
    }

    // Simple JSON parsing: find all {"q":X,"r":Y} patterns
    // This is a simplified parser that handles the expected format: [{"q":0,"r":0},...]
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() {
        // Look for opening brace
        if chars[i] == '{' {
            let mut q_value: Option<i32> = None;
            let mut r_value: Option<i32> = None;
            
            i += 1;
            while i < chars.len() && chars[i] != '}' {
                // Look for "q" or "r" followed by colon and number
                if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'q' && chars[i + 2] == '"' {
                    i += 3;
                    // Skip colon and whitespace
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    // Parse number
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            q_value = Some(num);
                        }
                    }
                } else if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'r' && chars[i + 2] == '"' {
                    i += 3;
                    // Skip colon and whitespace
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    // Parse number
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            r_value = Some(num);
                        }
                    }
                } else {
                    i += 1;
                }
            }
            
            if let (Some(q), Some(r)) = (q_value, r_value) {
                roads.push((q, r));
            }
        }
        i += 1;
    }

    if roads.is_empty() {
        return true;
    }

    if roads.len() == 1 {
        // Single road - check if it has at least one road neighbor
        // For single road, we consider it valid (can't check neighbors without more context)
        return true;
    }

    // Convert to HashSet for O(1) lookups
    let roads_set: HashSet<(i32, i32)> = roads.iter().cloned().collect();

    // Use first road as source
    let source = roads[0];

    // Check if all other roads are reachable from source using A*
    for road in roads.iter().skip(1) {
        let path_length = hex_astar_path(source.0, source.1, road.0, road.1, &roads_set);
        if path_length == -1 {
            return false; // Unreachable road found
        }
    }

    true // All roads reachable from source
}

/// Cube coordinate structure
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CubeCoord {
    q: i32,
    r: i32,
    s: i32,
}

/// Cube directions for hex grid navigation
const CUBE_DIRECTIONS: [CubeCoord; 6] = [
    CubeCoord { q: 1, r: 0, s: -1 },   // Direction 0
    CubeCoord { q: 1, r: -1, s: 0 },   // Direction 1
    CubeCoord { q: 0, r: -1, s: 1 },   // Direction 2
    CubeCoord { q: -1, r: 0, s: 1 },  // Direction 3
    CubeCoord { q: -1, r: 1, s: 0 },  // Direction 4
    CubeCoord { q: 0, r: 1, s: -1 },  // Direction 5
];

/// Add two cube coordinates
fn cube_add(a: CubeCoord, b: CubeCoord) -> CubeCoord {
    CubeCoord {
        q: a.q + b.q,
        r: a.r + b.r,
        s: a.s + b.s,
    }
}

/// Scale a cube coordinate by a factor
fn cube_scale(hex: CubeCoord, factor: i32) -> CubeCoord {
    CubeCoord {
        q: hex.q * factor,
        r: hex.r * factor,
        s: hex.s * factor,
    }
}

/// Get cube neighbor in specified direction (0-5)
fn cube_neighbor(cube: CubeCoord, direction: usize) -> CubeCoord {
    cube_add(cube, CUBE_DIRECTIONS[direction % 6])
}

/// Generate ring of tiles at specific layer (radius) around center
fn cube_ring(center: CubeCoord, radius: i32) -> Vec<CubeCoord> {
    if radius == 0 {
        return vec![center];
    }
    
    let mut results = Vec::new();
    
    // Start at the first hex of the ring by moving from the center
    // Move 'radius' steps in direction 4 (CUBE_DIRECTIONS[4])
    let mut current_hex = cube_add(center, cube_scale(CUBE_DIRECTIONS[4], radius));
    
    // Traverse the six sides of the hexagonal ring
    for i in 0..6 {
        // For each side, take 'radius' steps in the current direction
        for _j in 0..radius {
            results.push(current_hex);
            current_hex = cube_neighbor(current_hex, i);
        }
    }
    
    results
}

/// Generate hexagon grid up to max_layer
/// Returns all hex coordinates within the hexagon pattern
/// Matches TypeScript implementation using cube coordinates
fn generate_hex_grid(max_layer: i32, center_q: i32, center_r: i32) -> Vec<HexCoord> {
    let mut grid_set = HashSet::new();
    let center_cube = CubeCoord {
        q: center_q,
        r: center_r,
        s: -center_q - center_r,
    };
    
    // Generate grid from center outwards, adding one ring at a time
    for layer in 0..=max_layer {
        let ring = cube_ring(center_cube, layer);
        for cube in ring {
            // Use tuple of coordinates as hashable key for the set
            grid_set.insert((cube.q, cube.r, cube.s));
        }
    }
    
    // Convert set to array of HexCoord, verifying cube coordinate constraint
    let mut grid = Vec::new();
    for (q, r, s) in grid_set {
        // Verify cube coordinate is valid (q + r + s = 0)
        if q + r + s == 0 {
            grid.push(HexCoord { q, r });
        }
    }
    
    grid
}

/// Generate Voronoi regions for specified tile types
/// 
/// **Learning Point**: Generates seed points for each region type and assigns
/// each hex tile to the nearest seed point, creating Voronoi regions.
/// Returns JSON string with array of {q, r, tileType} objects.
/// 
/// @param max_layer - Maximum layer of hexagon (determines grid size)
/// @param center_q - Center q coordinate
/// @param center_r - Center r coordinate
/// @param forest_seeds - Number of forest region seeds
/// @param water_seeds - Number of water region seeds
/// @param grass_seeds - Number of grass region seeds
/// @returns JSON string with array of pre-constraints: [{"q":0,"r":0,"tileType":3},...]
#[wasm_bindgen]
pub fn generate_voronoi_regions(
    max_layer: i32,
    center_q: i32,
    center_r: i32,
    forest_seeds: i32,
    water_seeds: i32,
    grass_seeds: i32,
) -> String {
    // Generate hex grid
    let hex_grid = generate_hex_grid(max_layer, center_q, center_r);
    
    // Early return pattern matching for error cases
    let hex_vec: Vec<(i32, i32)> = match hex_grid.as_slice() {
        [] => {
            // If grid is empty, return at least one default entry
            return r#"[{"q":0,"r":0,"tileType":0}]"#.to_string();
        },
        _ => hex_grid.iter().map(|h| (h.q, h.r)).collect(),
    };
    
    let hex_count = hex_vec.len();
    match hex_count {
        0 => {
            // If hex_vec is empty, return at least one default entry
            return r#"[{"q":0,"r":0,"tileType":0}]"#.to_string();
        },
        _ => {},
    }
    
    // Generate seed points by sampling from actual hex grid coordinates
    // Use deterministic selection with prime multiplier for good distribution
    // This ensures seeds are ALWAYS generated reliably
    let mut seeds: Vec<VoronoiSeed> = Vec::new();
    let mut seed_counter: usize = 0;
    
    // Generate forest seeds
    // Ensure we have at least 0 seeds (handle negative values)
    let forest_count = if forest_seeds > 0 { forest_seeds as usize } else { 0 };
    for i in 0..forest_count {
        seed_counter += 1;
        // Use deterministic selection: (counter * prime) % count for good distribution
        // Prime 7919 provides good pseudo-random distribution
        let index = ((seed_counter * 7919) + (i * 997)) % hex_count;
        // Bounds check (should always pass due to modulo, but be safe)
        if index < hex_vec.len() {
            let (q, r) = hex_vec[index];
            seeds.push(VoronoiSeed {
                q,
                r,
                tile_type: TileType::Forest,
            });
        }
    }
    
    // Generate water seeds
    let water_count = if water_seeds > 0 { water_seeds as usize } else { 0 };
    for i in 0..water_count {
        seed_counter += 1;
        let index = ((seed_counter * 7919) + (i * 997)) % hex_count;
        if index < hex_vec.len() {
            let (q, r) = hex_vec[index];
            seeds.push(VoronoiSeed {
                q,
                r,
                tile_type: TileType::Water,
            });
        }
    }
    
    // Generate grass seeds
    let grass_count = if grass_seeds > 0 { grass_seeds as usize } else { 0 };
    for i in 0..grass_count {
        seed_counter += 1;
        let index = ((seed_counter * 7919) + (i * 997)) % hex_count;
        if index < hex_vec.len() {
            let (q, r) = hex_vec[index];
            seeds.push(VoronoiSeed {
                q,
                r,
                tile_type: TileType::Grass,
            });
        }
    }
    
    // CRITICAL: If no seeds were generated, force generation of at least one grass seed
    // This should never happen with positive seed counts, but ensures function always works
    match seeds.as_slice() {
        [] => {
            match hex_vec.first() {
                Some(&(q, r)) => {
                    seeds.push(VoronoiSeed {
                        q,
                        r,
                        tile_type: TileType::Grass,
                    });
                },
                None => return r#"[{"q":0,"r":0,"tileType":0}]"#.to_string(),
            }
        },
        _ => {},
    }
    
    // Assign each hex to nearest seed and build JSON
    // Ensure seeds is not empty (should be guaranteed by fallback above)
    let seeds_ref = match seeds.as_slice() {
        [] => return r#"[{"q":0,"r":0,"tileType":0}]"#.to_string(),
        s => s,
    };
    
    let mut json_parts = Vec::new();
    for hex in &hex_grid {
        let nearest_seed = seeds_ref.iter()
            .min_by_key(|seed| hex_distance(hex.q, hex.r, seed.q, seed.r));
        
        match nearest_seed {
            Some(seed) => {
                json_parts.push(format!(
                    r#"{{"q":{},"r":{},"tileType":{}}}"#,
                    hex.q, hex.r, seed.tile_type as i32
                ));
            },
            None => {},
        }
    }
    
    // If json_parts is empty (shouldn't happen), return at least one entry from first seed
    let json_parts = match json_parts.as_slice() {
        [] => {
            match (seeds_ref.first(), hex_grid.first()) {
                (Some(first_seed), _) => vec![format!(
                    r#"{{"q":{},"r":{},"tileType":{}}}"#,
                    first_seed.q, first_seed.r, first_seed.tile_type as i32
                )],
                (None, Some(first_hex)) => vec![format!(
                    r#"{{"q":{},"r":{},"tileType":0}}"#,
                    first_hex.q, first_hex.r
                )],
                (None, None) => return r#"[{"q":666,"r":666,"tileType":0}]"#.to_string(),
            }
        },
        parts => parts.to_vec(),
    };
    
    // Final safety check - ensure we never return empty array
    match json_parts.as_slice() {
        [] => return r#"[{"q":555,"r":555,"tileType":0}]"#.to_string(),
        _ => {},
    }
    
    let result = format!("[{}]", json_parts.join(","));
    // Final check - if result is somehow "[]", return test value
    match result.as_str() {
        "[]" => r#"[{"q":444,"r":444,"tileType":0}]"#.to_string(),
        _ => result,
    }
}

static WFC_STATE: LazyLock<Mutex<WfcState>> = LazyLock::new(|| Mutex::new(WfcState::new()));

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Get WASM module version for debugging and cache verification
/// 
/// Returns a version string that can be used to verify which WASM build is loaded.
/// Update this version when making significant changes to help debug caching issues.
#[wasm_bindgen]
pub fn get_wasm_version() -> String {
    "1.1.0-20250102-performance".to_string()
}

/// Generate a simplified layout using pre-constraints
/// 
/// **Learning Point**: This implements a simple algorithm:
/// 1. Apply pre-constraints to grid (all tile types set by TypeScript)
/// 2. Fill any remaining empty cells with grass (shouldn't happen if pre-constraints are complete)
#[wasm_bindgen]
pub fn generate_layout() {
    let mut state = WFC_STATE.lock().unwrap();
    state.clear();
    
    // Step 1: Apply pre-constraints to grid
    // Pre-constraints take absolute precedence - TypeScript sets all tiles
    // Collect pre-constraints into a vector first to avoid borrow checker issues
    let pre_constraints: Vec<((i32, i32), TileType)> = state.pre_constraints.iter().map(|((q, r), tile_type)| ((*q, *r), *tile_type)).collect();
    for ((q, r), tile_type) in pre_constraints {
        state.grid.insert((q, r), tile_type);
    }
    
    // Step 2: Fill any remaining empty cells with grass (shouldn't be needed if pre-constraints are complete)
    // This is a safety fallback
}

/// Get tile type at a specific hex grid position
/// 
/// **Learning Point**: This function is called from TypeScript to get the tile
/// at a specific hex position for rendering. Returns -1 if position is invalid or empty.
/// 
/// @param q - Hex column coordinate (axial, 0-49)
/// @param r - Hex row coordinate (axial, 0-49)
/// @returns Tile type as i32, or -1 if invalid/empty
#[wasm_bindgen]
pub fn get_tile_at(q: i32, r: i32) -> i32 {
    let state = WFC_STATE.lock().unwrap();
    if let Some(tile) = state.get_tile(q, r) {
        tile as i32
    } else {
        -1
    }
}

/// Clear the current layout
/// 
/// **Learning Point**: This resets the grid to all empty cells. Called when
/// the user clicks "Recompute Wave Collapse" to start fresh.
#[wasm_bindgen]
pub fn clear_layout() {
    let mut state = WFC_STATE.lock().unwrap();
    state.clear();
}

/// Set a pre-constraint at a specific hex position
/// 
/// **Learning Point**: Pre-constraints allow external systems to set specific tiles.
/// This enables guided generation based on high-level layout descriptions.
/// 
/// @param q - Hex column coordinate (axial q)
/// @param r - Hex row coordinate (axial r)
/// @param tile_type - Tile type as i32 (0-4, matching TileType enum)
/// @returns true if constraint was set successfully, false if tile type is invalid
#[wasm_bindgen]
pub fn set_pre_constraint(q: i32, r: i32, tile_type: i32) -> bool {
    let mut state = WFC_STATE.lock().unwrap();
    
    // Convert i32 to TileType
    let tile = match tile_type {
        0 => TileType::Grass,
        1 => TileType::Building,
        2 => TileType::Road,
        3 => TileType::Forest,
        4 => TileType::Water,
        _ => return false, // Invalid tile type
    };
    
    state.set_pre_constraint(q, r, tile)
}

/// Clear all pre-constraints
/// 
/// **Learning Point**: This clears all pre-constraints, allowing WFC to generate
/// completely random layouts again. Useful for resetting after text-guided generation.
#[wasm_bindgen]
pub fn clear_pre_constraints() {
    let mut state = WFC_STATE.lock().unwrap();
    state.clear_pre_constraints();
}

/// Get statistics about the current grid
/// 
/// **Learning Point**: This function iterates over the hash map to count all tile types.
/// Returns a JSON string with counts for each tile type.
/// Follows the pattern from wasm-agent-tools - builds JSON manually without serde
/// to keep WASM size small.
/// 
/// @returns JSON string with tile counts: {"grass":X,"building":Y,"road":Z,"forest":A,"water":B,"total":C}
#[wasm_bindgen]
pub fn get_stats() -> String {
    let state = WFC_STATE.lock().unwrap();
    
    let mut grass = 0;
    let mut building = 0;
    let mut road = 0;
    let mut forest = 0;
    let mut water = 0;
    
    for tile_type in state.grid.values() {
        match tile_type {
            TileType::Grass => grass += 1,
            TileType::Building => building += 1,
            TileType::Road => road += 1,
            TileType::Forest => forest += 1,
            TileType::Water => water += 1,
        }
    }
    
    let total = grass + building + road + forest + water;
    
    format!(
        r#"{{"grass":{},"building":{},"road":{},"forest":{},"water":{},"total":{}}}"#,
        grass, building, road, forest, water, total
    )
}

/// Find nearest point in connected set to a given point
/// Returns the nearest point and its distance
fn find_nearest_in_set(
    point: (i32, i32),
    connected_set: &HashSet<(i32, i32)>,
) -> Option<((i32, i32), i32)> {
    if connected_set.is_empty() {
        return None;
    }
    
    let mut nearest: Option<(i32, i32)> = None;
    let mut min_distance = i32::MAX;
    
    for &connected_point in connected_set {
        let dist = hex_distance(point.0, point.1, connected_point.0, connected_point.1);
        if dist < min_distance {
            min_distance = dist;
            nearest = Some(connected_point);
        }
    }
    
    nearest.map(|n| (n, min_distance))
}

/// Parse path JSON and return vector of coordinates
/// Format: [{"q":0,"r":0},{"q":1,"r":0},...]
fn parse_path_json(path_json: &str) -> Vec<(i32, i32)> {
    let mut path = Vec::new();
    
    if path_json == "null" || path_json.is_empty() {
        return path;
    }
    
    let trimmed = path_json.trim();
    if trimmed == "[]" || trimmed.len() < 3 {
        return path;
    }
    
    // Simple JSON parsing: find all {"q":X,"r":Y} patterns
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() {
        if chars[i] == '{' {
            let mut q_value: Option<i32> = None;
            let mut r_value: Option<i32> = None;
            
            i += 1;
            while i < chars.len() && chars[i] != '}' {
                if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'q' && chars[i + 2] == '"' {
                    i += 3;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            q_value = Some(num);
                        }
                    }
                } else if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'r' && chars[i + 2] == '"' {
                    i += 3;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            r_value = Some(num);
                        }
                    }
                } else {
                    i += 1;
                }
            }
            
            if let (Some(q), Some(r)) = (q_value, r_value) {
                path.push((q, r));
            }
        }
        i += 1;
    }
    
    path
}

/// Generate road network using true growing tree algorithm
/// 
/// Algorithm:
/// 1. Start with first seed point
/// 2. For each remaining seed: find nearest connected road, build A* path, add path
/// 3. For expansion: repeatedly find nearest unconnected valid terrain to any connected road,
///    build A* path, add path. Continue until target count reached.
/// 
/// This creates a true tree structure where every road is connected via a path,
/// not just adjacent (which would be flood fill).
/// 
/// @param seeds_json - JSON array of seed points: [{"q":0,"r":0},...]
/// @param valid_terrain_json - JSON array of valid terrain: [{"q":0,"r":0},...]
/// @param occupied_json - JSON array of occupied hexes: [{"q":0,"r":0},...]
/// @param target_count - Target number of roads to generate
/// @returns JSON array of road coordinates: [{"q":0,"r":0},...]
#[wasm_bindgen]
pub fn generate_road_network_growing_tree(
    seeds_json: String,
    valid_terrain_json: String,
    occupied_json: String,
    target_count: i32,
) -> String {
    // Parse inputs
    let seeds = parse_valid_terrain_json(&seeds_json);
    let valid_terrain = parse_valid_terrain_json(&valid_terrain_json);
    let occupied = parse_valid_terrain_json(&occupied_json);
    
    // Build valid terrain set (valid terrain minus occupied)
    let mut valid_terrain_set = HashSet::new();
    for &hex in &valid_terrain {
        if !occupied.contains(&hex) {
            valid_terrain_set.insert(hex);
        }
    }
    
    // Convert valid terrain to JSON for hex_astar calls
    let mut valid_terrain_vec: Vec<(i32, i32)> = valid_terrain_set.iter().cloned().collect();
    valid_terrain_vec.sort();
    let mut valid_terrain_json_parts = Vec::new();
    for (q, r) in &valid_terrain_vec {
        valid_terrain_json_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    let valid_terrain_json_for_astar = format!("[{}]", valid_terrain_json_parts.join(","));
    
    // Connected set: roads in the network
    let mut connected: HashSet<(i32, i32)> = HashSet::new();
    
    // Unconnected set: valid terrain not yet roads
    let mut unconnected: HashSet<(i32, i32)> = valid_terrain_set.clone();
    
    // Phase 1: Connect seed points
    if !seeds.is_empty() {
        let first_seed = seeds.iter().next().copied();
        if let Some(seed) = first_seed {
            if valid_terrain_set.contains(&seed) {
                connected.insert(seed);
                unconnected.remove(&seed);
            }
        }
        
        // Connect remaining seeds
        for seed in seeds.iter().skip(1) {
            if !valid_terrain_set.contains(seed) {
                continue;
            }
            
            if connected.is_empty() {
                // No connected roads yet, add seed directly
                connected.insert(*seed);
                unconnected.remove(seed);
                continue;
            }
            
            // Find nearest connected road
            if let Some((nearest_road, _)) = find_nearest_in_set(*seed, &connected) {
                // Build path from nearest road to seed
                let path_json = hex_astar(
                    nearest_road.0,
                    nearest_road.1,
                    seed.0,
                    seed.1,
                    valid_terrain_json_for_astar.clone(),
                );
                
                if path_json != "null" && !path_json.is_empty() {
                    let path = parse_path_json(&path_json);
                    // Add all path hexes to connected
                    for path_hex in path {
                        connected.insert(path_hex);
                        unconnected.remove(&path_hex);
                    }
                }
            }
        }
    }
    
    // Phase 2: Expand to target density using growing tree
    while (connected.len() as i32) < target_count && !unconnected.is_empty() {
        let mut best_unconnected: Option<(i32, i32)> = None;
        let mut best_connected: Option<(i32, i32)> = None;
        let mut min_distance = i32::MAX;
        
        // Find nearest unconnected point to any connected road
        for &unconnected_point in &unconnected {
            if let Some((nearest_road, distance)) = find_nearest_in_set(unconnected_point, &connected) {
                if distance < min_distance {
                    min_distance = distance;
                    best_unconnected = Some(unconnected_point);
                    best_connected = Some(nearest_road);
                }
            }
        }
        
        // Build path and add to network
        if let (Some(unconnected_point), Some(connected_road)) = (best_unconnected, best_connected) {
            let path_json = hex_astar(
                connected_road.0,
                connected_road.1,
                unconnected_point.0,
                unconnected_point.1,
                valid_terrain_json_for_astar.clone(),
            );
            
            if path_json != "null" && !path_json.is_empty() {
                let path = parse_path_json(&path_json);
                // Add all path hexes to connected
                for path_hex in path {
                    connected.insert(path_hex);
                    unconnected.remove(&path_hex);
                }
            } else {
                // Can't reach this point, remove it from unconnected
                unconnected.remove(&unconnected_point);
            }
        } else {
            // No more reachable points
            break;
        }
    }
    
    // Convert connected set to JSON array
    let mut road_vec: Vec<(i32, i32)> = connected.iter().cloned().collect();
    road_vec.sort();
    let mut json_parts = Vec::new();
    for (q, r) in road_vec {
        json_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    
    format!("[{}]", json_parts.join(","))
}

/// Calculate chunk radius for distance threshold calculations
/// The chunk radius is the distance from chunk center to the outer boundary
/// 
/// @param rings - Number of rings per chunk
/// @returns Chunk radius in hex distance units
#[wasm_bindgen]
pub fn calculate_chunk_radius(rings: i32) -> i32 {
    rings
}

/// Calculate chunk neighbor positions using offset vector rotation
/// Returns exactly 6 neighbor hex coordinates, one in each of the 6 directions
/// 
/// Uses the offset vector (rings, rings+1) for rings>0, or (1, 0) for rings=0, and rotates
/// it 60 degrees clockwise 6 times. This ensures chunks are packed without gaps - 
/// each direction has exactly one neighbor. The outer boundaries of adjacent chunks touch.
/// 
/// @param center_q - Center q coordinate
/// @param center_r - Center r coordinate
/// @param rings - Number of rings per chunk
/// @returns JSON string with array of 6 neighbor coordinates: [{"q":0,"r":0},...]
#[wasm_bindgen]
pub fn calculate_chunk_neighbors(center_q: i32, center_r: i32, rings: i32) -> String {
    let mut neighbors = Vec::new();
    
    // Base offset vector: (rings, rings+1) for rings>0, or (1, 0) for rings=0
    let (mut offset_q, mut offset_r) = if rings == 0 {
        (1, 0)
    } else {
        (rings, rings + 1)
    };
    
    // Rotate the starting offset by -120 degrees (4 steps clockwise) to correct angular alignment
    // This compensates for the 120-degree offset in the coordinate system
    for _i in 0..4 {
        let next_q = offset_q + offset_r;
        let next_r = -offset_q;
        offset_q = next_q;
        offset_r = next_r;
    }
    
    // Rotate the offset vector 60 degrees clockwise 6 times
    // Rotation formula in axial coordinates for clockwise: (q, r) -> (q+r, -q)
    let mut current_q = offset_q;
    let mut current_r = offset_r;
    
    for _i in 0..6 {
        // Add the current offset to the center
        neighbors.push((center_q + current_q, center_r + current_r));
        
        // Rotate 60 degrees clockwise: (q, r) -> (q+r, -q)
        let next_q = current_q + current_r;
        let next_r = -current_q;
        current_q = next_q;
        current_r = next_r;
    }
    
    // Convert to JSON
    let mut json_parts = Vec::new();
    for (q, r) in neighbors {
        json_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    
    format!("[{}]", json_parts.join(","))
}

/// Find the immediate neighbor chunk of the current chunk that is nearest to the current tile
/// Only considers the 6 immediate neighbors of the current chunk
/// 
/// @param current_chunk_q - Hex q coordinate of current chunk
/// @param current_chunk_r - Hex r coordinate of current chunk
/// @param current_tile_q - Hex q coordinate of current tile
/// @param current_tile_r - Hex r coordinate of current tile
/// @param rings - Number of rings per chunk
/// @param existing_chunks_json - JSON array of existing chunk positions: [{"q":0,"r":0},...]
/// @returns JSON string with nearest neighbor info: {"neighbor":{"q":0,"r":0},"distance":1.5,"isInstantiated":true} or "null"
#[wasm_bindgen]
pub fn find_nearest_neighbor_chunk(
    current_chunk_q: i32,
    current_chunk_r: i32,
    current_tile_q: i32,
    current_tile_r: i32,
    rings: i32,
    existing_chunks_json: String,
) -> String {
    // Parse existing chunks
    let existing_chunks = parse_valid_terrain_json(&existing_chunks_json);
    
    // Calculate immediate neighbors
    let neighbors_json = calculate_chunk_neighbors(current_chunk_q, current_chunk_r, rings);
    let neighbors = parse_valid_terrain_json(&neighbors_json);
    
    if neighbors.is_empty() {
        return "null".to_string();
    }
    
    // Find which of the immediate neighbors is closest to the current tile (in hex distance)
    let mut nearest_neighbor: Option<(i32, i32)> = None;
    let mut min_distance = i32::MAX;
    
    for neighbor_pos in &neighbors {
        let hex_dist = hex_distance(current_tile_q, current_tile_r, neighbor_pos.0, neighbor_pos.1);
        
        if hex_dist < min_distance {
            min_distance = hex_dist;
            nearest_neighbor = Some(*neighbor_pos);
        }
    }
    
    if let Some(neighbor) = nearest_neighbor {
        let is_instantiated = existing_chunks.contains(&neighbor);
        // Return distance as hex distance (TypeScript will convert to world distance if needed)
        format!(
            r#"{{"neighbor":{{"q":{},"r":{}}},"distance":{},"isInstantiated":{}}}"#,
            neighbor.0, neighbor.1, min_distance, is_instantiated
        )
    } else {
        "null".to_string()
    }
}

/// Disable chunks that are more than max_distance away from the current chunk
/// All chunks, including the origin chunk, are subject to the distance threshold
/// 
/// @param current_chunk_q - Hex q coordinate of current chunk
/// @param current_chunk_r - Hex r coordinate of current chunk
/// @param all_chunks_json - JSON array of all chunk positions with enabled state: [{"q":0,"r":0,"enabled":true},...]
/// @param max_distance - Maximum hex distance threshold
/// @returns JSON string with chunks to enable/disable: {"toDisable":[{"q":0,"r":0},...],"toEnable":[{"q":0,"r":0},...]}
#[wasm_bindgen]
pub fn disable_distant_chunks(
    current_chunk_q: i32,
    current_chunk_r: i32,
    all_chunks_json: String,
    max_distance: i32,
) -> String {
    // Parse chunks with enabled state
    // Format: [{"q":0,"r":0,"enabled":true},...]
    let mut chunks: Vec<(i32, i32, bool)> = Vec::new();
    
    let trimmed = all_chunks_json.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return r#"{"toDisable":[],"toEnable":[]}"#.to_string();
    }
    
    // Simple JSON parsing: find all {"q":X,"r":Y,"enabled":Z} patterns
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() {
        if chars[i] == '{' {
            let mut q_value: Option<i32> = None;
            let mut r_value: Option<i32> = None;
            let mut enabled_value: Option<bool> = None;
            
            i += 1;
            while i < chars.len() && chars[i] != '}' {
                // Look for "q", "r", or "enabled"
                if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'q' && chars[i + 2] == '"' {
                    i += 3;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            q_value = Some(num);
                        }
                    }
                } else if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'r' && chars[i + 2] == '"' {
                    i += 3;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            r_value = Some(num);
                        }
                    }
                } else if i + 9 < chars.len() && chars[i] == '"' && chars[i + 1] == 'e' && chars[i + 2] == 'n' 
                    && chars[i + 3] == 'a' && chars[i + 4] == 'b' && chars[i + 5] == 'l' 
                    && chars[i + 6] == 'e' && chars[i + 7] == 'd' && chars[i + 8] == '"' {
                    i += 9;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() {
                        if i + 4 < chars.len() && chars[i] == 't' && chars[i + 1] == 'r' 
                            && chars[i + 2] == 'u' && chars[i + 3] == 'e' {
                            enabled_value = Some(true);
                            i += 4;
                        } else if i + 5 < chars.len() && chars[i] == 'f' && chars[i + 1] == 'a' 
                            && chars[i + 2] == 'l' && chars[i + 3] == 's' && chars[i + 4] == 'e' {
                            enabled_value = Some(false);
                            i += 5;
                        }
                    }
                } else {
                    i += 1;
                }
            }
            
            if let (Some(q), Some(r), Some(enabled)) = (q_value, r_value, enabled_value) {
                chunks.push((q, r, enabled));
            }
        }
        i += 1;
    }
    
    // Calculate which chunks to disable/enable
    let mut to_disable: Vec<(i32, i32)> = Vec::new();
    let mut to_enable: Vec<(i32, i32)> = Vec::new();
    
    for (chunk_q, chunk_r, currently_enabled) in chunks {
        let distance = hex_distance(current_chunk_q, current_chunk_r, chunk_q, chunk_r);
        
        if distance > max_distance {
            if currently_enabled {
                to_disable.push((chunk_q, chunk_r));
            }
        } else {
            if !currently_enabled {
                to_enable.push((chunk_q, chunk_r));
            }
        }
    }
    
    // Build JSON response
    let mut disable_parts = Vec::new();
    for (q, r) in &to_disable {
        disable_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    
    let mut enable_parts = Vec::new();
    for (q, r) in &to_enable {
        enable_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    
    format!(
        r#"{{"toDisable":[{}],"toEnable":[{}]}}"#,
        disable_parts.join(","),
        enable_parts.join(",")
    )
}

/// Batch query tile types for multiple hex coordinates
/// Returns JSON array with tile types: [{"q":0,"r":0,"tileType":1},...]
/// 
/// @param hex_coords_json - JSON array of hex coordinates: [{"q":0,"r":0},...]
/// @returns JSON array with tile types for each coordinate
#[wasm_bindgen]
pub fn batch_get_tile_types(hex_coords_json: String) -> String {
    let state = WFC_STATE.lock().unwrap();
    
    // Parse hex coordinates
    let hex_coords = parse_valid_terrain_json(&hex_coords_json);
    
    let mut json_parts = Vec::new();
    for (q, r) in hex_coords {
        if let Some(tile) = state.get_tile(q, r) {
            json_parts.push(format!(
                r#"{{"q":{},"r":{},"tileType":{}}}"#,
                q, r, tile as i32
            ));
        }
    }
    
    format!("[{}]", json_parts.join(","))
}

/// Calculate which chunk contains a given tile
/// Returns chunk position that contains the tile, or null if not found
/// 
/// @param tile_q - Hex q coordinate of the tile
/// @param tile_r - Hex r coordinate of the tile
/// @param rings - Number of rings per chunk
/// @param chunk_positions_json - JSON array of chunk positions: [{"q":0,"r":0},...]
/// @returns JSON string with chunk position: {"q":0,"r":0} or "null"
#[wasm_bindgen]
pub fn calculate_chunk_for_tile(
    tile_q: i32,
    tile_r: i32,
    rings: i32,
    chunk_positions_json: String,
) -> String {
    // Parse chunk positions
    let chunk_positions = parse_valid_terrain_json(&chunk_positions_json);
    
    if chunk_positions.is_empty() {
        return "null".to_string();
    }
    
    let mut closest_chunk: Option<(i32, i32)> = None;
    let mut min_distance = i32::MAX;
    
    // Find chunk whose center is closest to the tile and within the chunk's boundary
    for chunk_pos in &chunk_positions {
        let distance = hex_distance(tile_q, tile_r, chunk_pos.0, chunk_pos.1);
        
        // If tile is exactly at chunk center, return immediately
        if distance == 0 {
            return format!(r#"{{"q":{},"r":{}}}"#, chunk_pos.0, chunk_pos.1);
        }
        
        // Check if tile is within this chunk's boundary (distance <= rings)
        if distance <= rings {
            // If multiple chunks contain this tile (overlap at boundaries), prefer the closest center
            if distance < min_distance {
                min_distance = distance;
                closest_chunk = Some(*chunk_pos);
            }
        }
    }
    
    if let Some(chunk) = closest_chunk {
        format!(r#"{{"q":{},"r":{}}}"#, chunk.0, chunk.1)
    } else {
        "null".to_string()
    }
}

/// Shuffle array in WASM using Fisher-Yates algorithm
/// Returns shuffled JSON array
/// 
/// @param array_json - JSON array to shuffle: [{"q":0,"r":0},...]
/// @returns Shuffled JSON array
#[wasm_bindgen]
pub fn shuffle_array(array_json: String) -> String {
    // Parse array
    let mut coords: Vec<(i32, i32)> = Vec::new();
    
    let trimmed = array_json.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return "[]".to_string();
    }
    
    // Simple JSON parsing: find all {"q":X,"r":Y} patterns
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();
    while i < chars.len() {
        if chars[i] == '{' {
            let mut q_value: Option<i32> = None;
            let mut r_value: Option<i32> = None;
            
            i += 1;
            while i < chars.len() && chars[i] != '}' {
                if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'q' && chars[i + 2] == '"' {
                    i += 3;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            q_value = Some(num);
                        }
                    }
                } else if i + 3 < chars.len() && chars[i] == '"' && chars[i + 1] == 'r' && chars[i + 2] == '"' {
                    i += 3;
                    while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                        i += 1;
                    }
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '-') {
                        let start = i;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() {
                            i += 1;
                        }
                        let num_str: String = chars[start..i].iter().collect();
                        if let Ok(num) = num_str.parse::<i32>() {
                            r_value = Some(num);
                        }
                    }
                } else {
                    i += 1;
                }
            }
            
            if let (Some(q), Some(r)) = (q_value, r_value) {
                coords.push((q, r));
            }
        }
        i += 1;
    }
    
    // Fisher-Yates shuffle using a simple PRNG
    // Use a deterministic seed based on array content for reproducibility
    let mut seed: u64 = 0;
    for (q, r) in &coords {
        seed = seed.wrapping_mul(31).wrapping_add((*q as u64).wrapping_mul(17).wrapping_add(*r as u64));
    }
    
    let mut rng_state = seed;
    let mut rng = || {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        rng_state
    };
    
    for i in (1..coords.len()).rev() {
        let j = (rng() % (i as u64 + 1)) as usize;
        coords.swap(i, j);
    }
    
    // Convert back to JSON
    let mut json_parts = Vec::new();
    for (q, r) in coords {
        json_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    
    format!("[{}]", json_parts.join(","))
}

/// Count adjacent roads for a given hex coordinate
/// 
/// @param hex_q - Hex q coordinate
/// @param hex_r - Hex r coordinate
/// @param road_network_json - JSON array of road coordinates: [{"q":0,"r":0},...]
/// @returns Number of adjacent roads (0-6)
#[wasm_bindgen]
pub fn count_adjacent_roads(hex_q: i32, hex_r: i32, road_network_json: String) -> i32 {
    let roads = parse_valid_terrain_json(&road_network_json);
    let roads_set: HashSet<(i32, i32)> = roads.iter().cloned().collect();
    
    let neighbors = get_hex_neighbors(hex_q, hex_r);
    let mut count = 0;
    
    for (nq, nr) in neighbors {
        if roads_set.contains(&(nq, nr)) {
            count += 1;
        }
    }
    
    count
}

/// Get all valid terrain hexes adjacent to existing roads
/// Returns array of hex coordinates that are:
/// - Adjacent to at least one road in the network
/// - On valid terrain (in valid_terrain_json)
/// - Not already occupied
/// 
/// @param road_network_json - JSON array of road coordinates: [{"q":0,"r":0},...]
/// @param valid_terrain_json - JSON array of valid terrain: [{"q":0,"r":0},...]
/// @param occupied_json - JSON array of occupied hexes: [{"q":0,"r":0},...]
/// @returns JSON array of adjacent valid terrain: [{"q":0,"r":0},...]
#[wasm_bindgen]
pub fn get_adjacent_valid_terrain(
    road_network_json: String,
    valid_terrain_json: String,
    occupied_json: String,
) -> String {
    let roads = parse_valid_terrain_json(&road_network_json);
    let valid_terrain = parse_valid_terrain_json(&valid_terrain_json);
    let occupied = parse_valid_terrain_json(&occupied_json);
    
    let roads_set: HashSet<(i32, i32)> = roads.iter().cloned().collect();
    let valid_terrain_set: HashSet<(i32, i32)> = valid_terrain.iter().cloned().collect();
    let occupied_set: HashSet<(i32, i32)> = occupied.iter().cloned().collect();
    
    let mut adjacent_hexes: HashSet<(i32, i32)> = HashSet::new();
    
    // For each road, find its neighbors
    for (road_q, road_r) in roads {
        let neighbors = get_hex_neighbors(road_q, road_r);
        for (nq, nr) in neighbors {
            let neighbor_key = (nq, nr);
            
            // Skip if already a road
            if roads_set.contains(&neighbor_key) {
                continue;
            }
            
            // Skip if occupied
            if occupied_set.contains(&neighbor_key) {
                continue;
            }
            
            // Check if this neighbor is in valid terrain
            if valid_terrain_set.contains(&neighbor_key) {
                adjacent_hexes.insert(neighbor_key);
            }
        }
    }
    
    // Convert to JSON
    let mut adjacent_vec: Vec<(i32, i32)> = adjacent_hexes.iter().cloned().collect();
    adjacent_vec.sort();
    
    let mut json_parts = Vec::new();
    for (q, r) in adjacent_vec {
        json_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    
    format!("[{}]", json_parts.join(","))
}

/// Generate building placement on valid terrain adjacent to roads
/// 
/// @param valid_terrain_json - JSON array of valid terrain: [{"q":0,"r":0},...]
/// @param road_network_json - JSON array of road coordinates: [{"q":0,"r":0},...]
/// @param occupied_json - JSON array of occupied hexes: [{"q":0,"r":0},...]
/// @param building_rules_json - JSON string with building rules: {"minAdjacentRoads":1}
/// @param target_count - Target number of buildings to place
/// @returns JSON array of building positions: [{"q":0,"r":0},...]
#[wasm_bindgen]
pub fn generate_building_placement(
    valid_terrain_json: String,
    road_network_json: String,
    occupied_json: String,
    building_rules_json: String,
    target_count: i32,
) -> String {
    let valid_terrain = parse_valid_terrain_json(&valid_terrain_json);
    let roads = parse_valid_terrain_json(&road_network_json);
    let occupied = parse_valid_terrain_json(&occupied_json);
    
    let roads_set: HashSet<(i32, i32)> = roads.iter().cloned().collect();
    let occupied_set: HashSet<(i32, i32)> = occupied.iter().cloned().collect();
    
    // Parse building rules
    let mut min_adjacent_roads = 1;
    let trimmed_rules = building_rules_json.trim();
    if !trimmed_rules.is_empty() && trimmed_rules != "{}" {
        // Simple JSON parsing for minAdjacentRoads
        let chars: Vec<char> = trimmed_rules.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if i + 18 < chars.len() && chars[i] == '"' && chars[i + 1] == 'm' && chars[i + 2] == 'i' 
                && chars[i + 3] == 'n' && chars[i + 4] == 'A' && chars[i + 5] == 'd' 
                && chars[i + 6] == 'j' && chars[i + 7] == 'a' && chars[i + 8] == 'c' 
                && chars[i + 9] == 'e' && chars[i + 10] == 'n' && chars[i + 11] == 't' 
                && chars[i + 12] == 'R' && chars[i + 13] == 'o' && chars[i + 14] == 'a' 
                && chars[i + 15] == 'd' && chars[i + 16] == 's' && chars[i + 17] == '"' {
                i += 18;
                while i < chars.len() && (chars[i] == ':' || chars[i] == ' ' || chars[i] == '\t') {
                    i += 1;
                }
                if i < chars.len() && chars[i].is_ascii_digit() {
                    let start = i;
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    let num_str: String = chars[start..i].iter().collect();
                    if let Ok(num) = num_str.parse::<i32>() {
                        min_adjacent_roads = num;
                    }
                }
                break;
            }
            i += 1;
        }
    }
    
    // Find available hexes for buildings
    let mut available_building_hexes: Vec<(i32, i32)> = Vec::new();
    
    for (terrain_q, terrain_r) in &valid_terrain {
        let terrain_key = (*terrain_q, *terrain_r);
        
        // Skip if occupied
        if occupied_set.contains(&terrain_key) {
            continue;
        }
        
        // Count adjacent roads
        let neighbors = get_hex_neighbors(*terrain_q, *terrain_r);
        let mut adjacent_road_count = 0;
        for (nq, nr) in neighbors {
            if roads_set.contains(&(nq, nr)) {
                adjacent_road_count += 1;
            }
        }
        
        // Check if meets minimum adjacent roads requirement
        if adjacent_road_count >= min_adjacent_roads {
            available_building_hexes.push(terrain_key);
        }
    }
    
    // Shuffle available building hexes
    if available_building_hexes.len() > 1 {
        // Use deterministic seed based on content
        let mut seed: u64 = 0;
        for (q, r) in &available_building_hexes {
            seed = seed.wrapping_mul(31).wrapping_add((*q as u64).wrapping_mul(17).wrapping_add(*r as u64));
        }
        
        let mut rng_state = seed;
        let mut rng = || {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            rng_state
        };
        
        for i in (1..available_building_hexes.len()).rev() {
            let j = (rng() % (i as u64 + 1)) as usize;
            available_building_hexes.swap(i, j);
        }
    }
    
    // Limit to target count
    let building_count = target_count.min(available_building_hexes.len() as i32);
    let selected_buildings = &available_building_hexes[0..(building_count as usize)];
    
    // Convert to JSON
    let mut json_parts = Vec::new();
    for (q, r) in selected_buildings {
        json_parts.push(format!(r#"{{"q":{},"r":{}}}"#, q, r));
    }
    
    format!("[{}]", json_parts.join(","))
}

/// Batch convert hex coordinates to world positions
/// 
/// @param hex_coords_json - JSON array of hex coordinates: [{"q":0,"r":0},...]
/// @param hex_size - Size of hexagon for coordinate conversion
/// @returns JSON array with world positions: [{"q":0,"r":0,"x":0.0,"z":0.0},...]
#[wasm_bindgen]
pub fn batch_hex_to_world(hex_coords_json: String, hex_size: f64) -> String {
    let hex_coords = parse_valid_terrain_json(&hex_coords_json);
    
    // Formula for pointy-top hexagons:
    // x = size * (√3 * q + √3/2 * r)
    // z = size * (3/2 * r)
    // Adjusted for the scaling factor used in TypeScript (hexSize / 1.34)
    let adjusted_hex_size = hex_size / 1.34;
    let sqrt3 = 3.0_f64.sqrt();
    
    let mut json_parts = Vec::new();
    for (q, r) in hex_coords {
        let q_f = q as f64;
        let r_f = r as f64;
        let x = adjusted_hex_size * (sqrt3 * 2.0 * q_f + sqrt3 * r_f);
        let z = adjusted_hex_size * (3.0 * r_f);
        
        json_parts.push(format!(
            r#"{{"q":{},"r":{},"x":{},"z":{}}}"#,
            q, r, x, z
        ));
    }
    
    format!("[{}]", json_parts.join(","))
}


