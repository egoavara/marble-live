//! Network topology visualization component for P2P debug page.
//!
//! Visualizes mesh groups with clustering layout using Canvas for high performance.
//! Supports pan (drag) and zoom (scroll) interactions without triggering React re-renders.
//! Fetches topology data directly from the server using GetRoomTopology API.

use std::collections::HashMap;
use marble_proto::room::{GetRoomTopologyRequest, PlayerAuth};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use yew::prelude::*;

use crate::hooks::use_grpc_room_service;

/// Network information for a peer instance (for visualization)
#[derive(Clone, PartialEq, Default)]
pub struct PeerNetworkInfo {
    /// Instance ID
    pub instance_id: u32,
    /// Player ID
    pub player_id: String,
    /// Connected peer IDs (player_id strings)
    pub connected_peers: Vec<String>,
    /// Connection state
    pub is_connected: bool,
    /// Mesh group ID (from topology)
    pub mesh_group: Option<u32>,
    /// Is this peer a bridge?
    pub is_bridge: bool,
}

/// Props for the NetworkVisualization component
#[derive(Properties, PartialEq)]
pub struct NetworkVisualizationProps {
    /// Room ID to fetch topology from
    pub room_id: String,
    /// Player ID for authentication
    pub player_id: String,
    /// Player secret for authentication
    pub player_secret: String,
    /// Auto-refresh interval in milliseconds (0 = disabled)
    #[prop_or(0)]
    pub auto_refresh_ms: u32,
    /// External refresh trigger - increment to force refresh
    #[prop_or(0)]
    pub refresh_trigger: u32,
}

/// Render state for pan/zoom (managed outside of Yew state for performance)
#[derive(Clone)]
struct RenderState {
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    is_dragging: bool,
    drag_start: (f64, f64),
    pan_start: (f64, f64),
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            is_dragging: false,
            drag_start: (0.0, 0.0),
            pan_start: (0.0, 0.0),
        }
    }
}

/// Layout information for a node
#[derive(Clone, PartialEq)]
struct NodeLayout {
    instance_id: u32,
    x: f64,
    y: f64,
    color: String,
    is_connected: bool,
    is_bridge: bool,
    connected_peer_count: usize,
}

/// Layout information for a group
#[derive(Clone, PartialEq)]
struct GroupLayout {
    group_id: u32,
    center_x: f64,
    center_y: f64,
    radius: f64,
    color: String,
    peer_count: usize,
    bridge_count: usize,
}

/// Layout information for a connection line
#[derive(Clone, PartialEq)]
struct LineLayout {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    color: String,
    is_inter_group: bool,
}

/// Pre-calculated layout for the entire network
#[derive(Clone, Default, PartialEq)]
struct NetworkLayout {
    nodes: Vec<NodeLayout>,
    groups: Vec<GroupLayout>,
    lines: Vec<LineLayout>,
    width: f64,
    height: f64,
}

/// Group colors for mesh groups
const GROUP_COLORS: &[&str] = &[
    "#667eea", // Blue
    "#4caf50", // Green
    "#ff9800", // Orange
    "#e91e63", // Pink
    "#00bcd4", // Cyan
    "#9c27b0", // Purple
    "#ff5722", // Deep Orange
    "#3f51b5", // Indigo
];

/// Calculate layout from network info
fn calculate_layout(network_info: &HashMap<u32, PeerNetworkInfo>) -> NetworkLayout {
    let peers: Vec<_> = network_info.values().collect();

    if peers.is_empty() {
        return NetworkLayout::default();
    }

    // Group peers by mesh_group
    let mut groups: HashMap<u32, Vec<&PeerNetworkInfo>> = HashMap::new();
    let mut ungrouped: Vec<&PeerNetworkInfo> = Vec::new();

    for peer in &peers {
        if let Some(group) = peer.mesh_group {
            groups.entry(group).or_default().push(peer);
        } else {
            ungrouped.push(peer);
        }
    }

    // Node sizing
    let node_radius = 22.0_f64;
    let node_spacing = 15.0_f64;
    let node_total = node_radius * 2.0 + node_spacing;

    // Calculate max peers in any group for sizing
    let max_peers_in_group = groups.values().map(|g| g.len()).max().unwrap_or(1).max(1);

    // Calculate cluster radius based on peer count
    let min_cluster_radius = (max_peers_in_group as f64 * node_total) / (2.0 * std::f64::consts::PI);
    let cluster_radius = min_cluster_radius.max(80.0);

    // Dynamic sizing
    let group_count = groups.len().max(1);
    let group_spacing = (cluster_radius * 2.0 + 100.0).max(250.0);
    let base_width = group_spacing * (group_count as f64 + 1.0);
    let base_height = (cluster_radius * 2.0 + 150.0).max(400.0);
    let center_y = base_height / 2.0;

    // Build position map
    let mut position_map: HashMap<u32, (f64, f64, String)> = HashMap::new();

    // Sort groups by group number
    let mut sorted_groups: Vec<_> = groups.iter().collect();
    sorted_groups.sort_by_key(|(k, _)| *k);

    let mut group_layouts: Vec<GroupLayout> = Vec::new();

    for (group_idx, (group_id, group_peers)) in sorted_groups.iter().enumerate() {
        let cluster_x = group_spacing * (group_idx as f64 + 1.0);
        let cluster_y = center_y;
        let color = GROUP_COLORS[(**group_id as usize) % GROUP_COLORS.len()].to_string();

        let peer_count = group_peers.len();
        let group_cluster_radius = if peer_count <= 1 {
            0.0
        } else {
            ((peer_count as f64 * node_total) / (2.0 * std::f64::consts::PI)).max(60.0)
        };

        let bridge_count = group_peers.iter().filter(|p| p.is_bridge).count();

        group_layouts.push(GroupLayout {
            group_id: **group_id,
            center_x: cluster_x,
            center_y: cluster_y,
            radius: group_cluster_radius.max(cluster_radius) + 30.0,
            color: color.clone(),
            peer_count,
            bridge_count,
        });

        for (i, peer) in group_peers.iter().enumerate() {
            let angle = if peer_count == 1 {
                0.0
            } else {
                (i as f64 / peer_count as f64) * 2.0 * std::f64::consts::PI
                    - std::f64::consts::PI / 2.0
            };
            let x = cluster_x + group_cluster_radius * angle.cos();
            let y = cluster_y + group_cluster_radius * angle.sin();
            position_map.insert(peer.instance_id, (x, y, color.clone()));
        }
    }

    // Place ungrouped peers at the bottom
    if !ungrouped.is_empty() {
        let ungrouped_y = base_height - 50.0;
        let ungrouped_spacing = base_width / (ungrouped.len() as f64 + 1.0);
        for (i, peer) in ungrouped.iter().enumerate() {
            let x = ungrouped_spacing * (i as f64 + 1.0);
            position_map.insert(peer.instance_id, (x, ungrouped_y, "#666".to_string()));
        }
    }

    // Build node layouts
    let mut node_layouts: Vec<NodeLayout> = Vec::new();
    for peer in &peers {
        if let Some((x, y, color)) = position_map.get(&peer.instance_id) {
            node_layouts.push(NodeLayout {
                instance_id: peer.instance_id,
                x: *x,
                y: *y,
                color: color.clone(),
                is_connected: peer.is_connected,
                is_bridge: peer.is_bridge,
                connected_peer_count: peer.connected_peers.len(),
            });
        }
    }

    // Generate connection lines
    let mut lines: Vec<LineLayout> = Vec::new();

    // Intra-group connections
    for (group_id, group_peers) in &groups {
        let color = GROUP_COLORS[(*group_id as usize) % GROUP_COLORS.len()].to_string();
        let connected_peers: Vec<_> = group_peers
            .iter()
            .filter(|p| p.is_connected && !p.connected_peers.is_empty())
            .collect();

        for i in 0..connected_peers.len() {
            let peer_a = connected_peers[i];
            let Some(&(x1, y1, _)) = position_map.get(&peer_a.instance_id) else {
                continue;
            };

            for j in (i + 1)..connected_peers.len() {
                let peer_b = connected_peers[j];
                let Some(&(x2, y2, _)) = position_map.get(&peer_b.instance_id) else {
                    continue;
                };
                lines.push(LineLayout {
                    x1,
                    y1,
                    x2,
                    y2,
                    color: color.clone(),
                    is_inter_group: false,
                });
            }
        }
    }

    // Inter-group connections
    let sorted_group_ids: Vec<_> = sorted_groups.iter().map(|(id, _)| **id).collect();

    for (group_idx, (_, group_peers)) in sorted_groups.iter().enumerate() {
        let bridge_peers: Vec<_> = group_peers
            .iter()
            .filter(|p| p.is_bridge && p.is_connected)
            .collect();

        let representative_peers: Vec<_> = if bridge_peers.is_empty() {
            group_peers
                .iter()
                .filter(|p| p.is_connected)
                .take(1)
                .collect()
        } else {
            bridge_peers
        };

        if representative_peers.is_empty() {
            continue;
        }

        for rep_peer in representative_peers {
            let Some(&(rep_x, rep_y, _)) = position_map.get(&rep_peer.instance_id) else {
                continue;
            };

            for (other_idx, other_group_id) in sorted_group_ids.iter().enumerate() {
                if other_idx <= group_idx {
                    continue;
                }

                let other_has_peers = groups.get(other_group_id).map_or(false, |peers| {
                    peers.iter().any(|p| p.is_connected)
                });

                if other_has_peers {
                    let other_center_x = group_spacing * (other_idx as f64 + 1.0);
                    let other_center_y = center_y;
                    lines.push(LineLayout {
                        x1: rep_x,
                        y1: rep_y,
                        x2: other_center_x,
                        y2: other_center_y,
                        color: "#ffd43b".to_string(),
                        is_inter_group: true,
                    });
                }
            }
        }
    }

    NetworkLayout {
        nodes: node_layouts,
        groups: group_layouts,
        lines,
        width: base_width,
        height: base_height,
    }
}

/// Render the network visualization to canvas
fn render_canvas(
    ctx: &CanvasRenderingContext2d,
    layout: &NetworkLayout,
    state: &RenderState,
    canvas_width: f64,
    canvas_height: f64,
) {
    // Clear canvas
    ctx.set_fill_style_str("#0d0d1a");
    ctx.fill_rect(0.0, 0.0, canvas_width, canvas_height);

    // Apply transform (pan/zoom)
    ctx.save();
    ctx.translate(state.pan_x, state.pan_y).ok();
    ctx.scale(state.zoom, state.zoom).ok();

    // Draw group backgrounds
    for group in &layout.groups {
        ctx.begin_path();
        ctx.arc(
            group.center_x,
            group.center_y,
            group.radius,
            0.0,
            2.0 * std::f64::consts::PI,
        )
        .ok();
        ctx.set_fill_style_str(&format!("{}1a", group.color)); // 10% opacity
        ctx.fill();

        // Group label
        ctx.set_fill_style_str(&group.color);
        ctx.set_font("bold 12px sans-serif");
        ctx.set_text_align("center");
        ctx.fill_text(
            &format!("Group {}", group.group_id),
            group.center_x,
            group.center_y - group.radius - 25.0,
        )
        .ok();

        // Peer count and bridge count
        ctx.set_fill_style_str("#888");
        ctx.set_font("10px sans-serif");
        let info_text = if group.bridge_count > 0 {
            format!("{} peers ({} bridge)", group.peer_count, group.bridge_count)
        } else {
            format!("{} peers", group.peer_count)
        };
        ctx.fill_text(&info_text, group.center_x, group.center_y - group.radius - 10.0)
            .ok();
    }

    // Draw inter-group lines first (behind)
    for line in layout.lines.iter().filter(|l| l.is_inter_group) {
        ctx.begin_path();
        ctx.move_to(line.x1, line.y1);
        ctx.line_to(line.x2, line.y2);
        ctx.set_stroke_style_str(&line.color);
        ctx.set_line_width(2.0);
        ctx.set_line_dash(&JsValue::from(js_sys::Array::of2(
            &JsValue::from(6.0),
            &JsValue::from(3.0),
        )))
        .ok();
        ctx.set_global_alpha(0.7);
        ctx.stroke();
        ctx.set_line_dash(&JsValue::from(js_sys::Array::new())).ok();
        ctx.set_global_alpha(1.0);
    }

    // Draw intra-group lines
    for line in layout.lines.iter().filter(|l| !l.is_inter_group) {
        ctx.begin_path();
        ctx.move_to(line.x1, line.y1);
        ctx.line_to(line.x2, line.y2);
        ctx.set_stroke_style_str(&line.color);
        ctx.set_line_width(2.0);
        ctx.set_global_alpha(0.5);
        ctx.stroke();
        ctx.set_global_alpha(1.0);
    }

    // Draw nodes
    let node_radius = 22.0;
    for node in &layout.nodes {
        // Bridge indicator (outer golden ring)
        if node.is_bridge {
            ctx.begin_path();
            ctx.arc(node.x, node.y, node_radius + 6.0, 0.0, 2.0 * std::f64::consts::PI)
                .ok();
            ctx.set_stroke_style_str("#ffd43b");
            ctx.set_line_width(3.0);
            ctx.stroke();
        }

        // Connection status ring
        ctx.begin_path();
        ctx.arc(node.x, node.y, node_radius + 2.0, 0.0, 2.0 * std::f64::consts::PI)
            .ok();
        ctx.set_stroke_style_str(if node.is_connected { "#69db7c" } else { "#666" });
        ctx.set_line_width(2.0);
        ctx.stroke();

        // Node circle
        ctx.begin_path();
        ctx.arc(node.x, node.y, node_radius, 0.0, 2.0 * std::f64::consts::PI)
            .ok();
        ctx.set_global_alpha(if node.is_connected { 1.0 } else { 0.4 });
        ctx.set_fill_style_str(&node.color);
        ctx.fill();
        ctx.set_global_alpha(1.0);

        // Peer number
        ctx.set_fill_style_str("#fff");
        ctx.set_font("bold 12px sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.fill_text(&format!("#{}", node.instance_id), node.x, node.y)
            .ok();

        // Peer count (below node)
        if node.is_connected {
            ctx.set_fill_style_str("#aaa");
            ctx.set_font("9px sans-serif");
            ctx.set_text_baseline("top");
            ctx.fill_text(
                &format!("{} peers", node.connected_peer_count),
                node.x,
                node.y + node_radius + 4.0,
            )
            .ok();
        }
    }

    ctx.restore();

    // Draw legend (fixed position, not affected by transform)
    draw_legend(ctx);

    // Draw zoom indicator
    ctx.set_fill_style_str("#666");
    ctx.set_font("10px sans-serif");
    ctx.set_text_align("right");
    ctx.set_text_baseline("top");
    ctx.fill_text(
        &format!("Zoom: {:.0}%", state.zoom * 100.0),
        canvas_width - 10.0,
        10.0,
    )
    .ok();
}

/// Draw the legend in fixed position
fn draw_legend(ctx: &CanvasRenderingContext2d) {
    let x = 10.0;
    let y = 10.0;

    ctx.set_fill_style_str("#888");
    ctx.set_font("10px sans-serif");
    ctx.set_text_align("left");
    ctx.set_text_baseline("top");
    ctx.fill_text("Legend:", x, y).ok();

    // Intra-group line
    ctx.begin_path();
    ctx.move_to(x, y + 20.0);
    ctx.line_to(x + 20.0, y + 20.0);
    ctx.set_stroke_style_str("#667eea");
    ctx.set_line_width(2.0);
    ctx.stroke();
    ctx.set_fill_style_str("#888");
    ctx.set_font("9px sans-serif");
    ctx.fill_text("Intra-group", x + 25.0, y + 16.0).ok();

    // Inter-group line
    ctx.begin_path();
    ctx.move_to(x, y + 35.0);
    ctx.line_to(x + 20.0, y + 35.0);
    ctx.set_stroke_style_str("#ffd43b");
    ctx.set_line_width(2.0);
    ctx.set_line_dash(&JsValue::from(js_sys::Array::of2(
        &JsValue::from(6.0),
        &JsValue::from(3.0),
    )))
    .ok();
    ctx.stroke();
    ctx.set_line_dash(&JsValue::from(js_sys::Array::new())).ok();
    ctx.fill_text("Bridge", x + 25.0, y + 31.0).ok();

    // Instructions
    ctx.set_fill_style_str("#555");
    ctx.set_font("8px sans-serif");
    ctx.fill_text("Scroll: Zoom | Drag: Pan", x, y + 50.0).ok();
}

/// Loading state enum
#[derive(Clone, PartialEq)]
enum LoadingState {
    Idle,
    Loading,
    Error(String),
    Loaded,
}

/// Network visualization component using Canvas for high performance
/// Fetches topology data from server using GetRoomTopology API
#[function_component(NetworkVisualization)]
pub fn network_visualization(props: &NetworkVisualizationProps) -> Html {
    let grpc = use_grpc_room_service();
    let canvas_ref = use_node_ref();

    // Network info state (fetched from server)
    let network_info = use_state(HashMap::<u32, PeerNetworkInfo>::new);
    let loading_state = use_state(|| LoadingState::Idle);
    let internal_refresh_trigger = use_state(|| 0u32);
    let has_loaded_once = use_state(|| false);

    // Track stable canvas dimensions (only grow, never shrink)
    let stable_canvas_size = use_mut_ref(|| (600.0_f64, 400.0_f64));

    // Combined refresh trigger (internal + external)
    let combined_trigger = *internal_refresh_trigger + props.refresh_trigger;

    // Render state (managed outside Yew for performance)
    let render_state = use_mut_ref(RenderState::default);

    // Fetch topology when room_id changes or refresh is triggered
    {
        let grpc = grpc.clone();
        let room_id = props.room_id.clone();
        let player_id = props.player_id.clone();
        let player_secret = props.player_secret.clone();
        let network_info = network_info.clone();
        let loading_state = loading_state.clone();
        let has_loaded_once = has_loaded_once.clone();

        use_effect_with(
            (room_id.clone(), combined_trigger),
            move |(room_id, _)| {
                let room_id = room_id.clone();
                if room_id.is_empty() {
                    loading_state.set(LoadingState::Idle);
                    network_info.set(HashMap::new());
                    return;
                }

                loading_state.set(LoadingState::Loading);

                spawn_local(async move {
                    let req = GetRoomTopologyRequest {
                        room_id: room_id.clone(),
                        player_auth: Some(PlayerAuth {
                            id: player_id,
                            secret: player_secret,
                        }),
                    };

                    match grpc.borrow_mut().get_room_topology(req).await {
                        Ok(resp) => {
                            let resp = resp.into_inner();
                            let mut map = HashMap::new();

                            for (idx, player_info) in resp.players.iter().enumerate() {
                                if let Some(topology) = &player_info.topology {
                                    let peer_info = PeerNetworkInfo {
                                        instance_id: idx as u32,
                                        player_id: player_info.player_id.clone(),
                                        mesh_group: Some(topology.mesh_group),
                                        is_bridge: topology.is_bridge,
                                        connected_peers: topology
                                            .connect_to
                                            .iter()
                                            .map(|c| c.player_id.clone())
                                            .collect(),
                                        is_connected: player_info.is_connected,
                                    };
                                    map.insert(idx as u32, peer_info);
                                }
                            }

                            network_info.set(map);
                            loading_state.set(LoadingState::Loaded);
                            has_loaded_once.set(true);
                        }
                        Err(e) => {
                            loading_state.set(LoadingState::Error(e.to_string()));
                        }
                    }
                });
            },
        );
    }

    // Auto-refresh effect
    {
        let internal_refresh_trigger = internal_refresh_trigger.clone();
        let auto_refresh_ms = props.auto_refresh_ms;
        let room_id = props.room_id.clone();

        use_effect_with(
            (auto_refresh_ms, room_id),
            move |(auto_refresh_ms, room_id)| {
                let interval_handle: Option<gloo::timers::callback::Interval> =
                    if *auto_refresh_ms == 0 || room_id.is_empty() {
                        None
                    } else {
                        let interval_ms = *auto_refresh_ms;
                        let trigger = internal_refresh_trigger.clone();

                        Some(gloo::timers::callback::Interval::new(interval_ms, move || {
                            trigger.set(*trigger + 1);
                        }))
                    };

                // Return cleanup closure
                move || drop(interval_handle)
            },
        );
    }

    // Calculate layout only when network_info changes
    let layout = use_memo((*network_info).clone(), |info| calculate_layout(info));

    // Canvas dimensions - only grow, never shrink to prevent flickering
    let (canvas_width, canvas_height) = {
        let new_width = layout.width.max(600.0);
        let new_height = layout.height.max(400.0);
        let mut size = stable_canvas_size.borrow_mut();
        if new_width > size.0 {
            size.0 = new_width;
        }
        if new_height > size.1 {
            size.1 = new_height;
        }
        (size.0, size.1)
    };

    // Initial render and re-render when layout changes
    {
        let canvas_ref = canvas_ref.clone();
        let layout = layout.clone();
        let render_state = render_state.clone();

        use_effect_with(layout.clone(), move |layout| {
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                if let Ok(Some(context)) = canvas.get_context("2d") {
                    if let Ok(ctx) = context.dyn_into::<CanvasRenderingContext2d>() {
                        let state = render_state.borrow();
                        render_canvas(
                            &ctx,
                            layout,
                            &state,
                            canvas.width() as f64,
                            canvas.height() as f64,
                        );
                    }
                }
            }
            || ()
        });
    }

    // Refresh button handler
    let on_refresh = {
        let internal_refresh_trigger = internal_refresh_trigger.clone();
        Callback::from(move |_: MouseEvent| {
            internal_refresh_trigger.set(*internal_refresh_trigger + 1);
        })
    };

    // Wheel handler for zoom
    let on_wheel = {
        let canvas_ref = canvas_ref.clone();
        let render_state = render_state.clone();
        let layout = layout.clone();

        Callback::from(move |e: WheelEvent| {
            e.prevent_default();

            let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() else {
                return;
            };

            let mut state = render_state.borrow_mut();
            let delta = e.delta_y();
            let zoom_factor = if delta > 0.0 { 0.9 } else { 1.1 };
            let new_zoom = (state.zoom * zoom_factor).clamp(0.25, 4.0);

            // Get mouse position for zoom towards cursor
            let rect = canvas.get_bounding_client_rect();
            let mouse_x = e.client_x() as f64 - rect.left();
            let mouse_y = e.client_y() as f64 - rect.top();

            // Adjust pan to zoom towards mouse position
            let zoom_ratio = new_zoom / state.zoom;
            state.pan_x = mouse_x - (mouse_x - state.pan_x) * zoom_ratio;
            state.pan_y = mouse_y - (mouse_y - state.pan_y) * zoom_ratio;
            state.zoom = new_zoom;

            // Re-render canvas directly
            if let Ok(Some(context)) = canvas.get_context("2d") {
                if let Ok(ctx) = context.dyn_into::<CanvasRenderingContext2d>() {
                    render_canvas(
                        &ctx,
                        &layout,
                        &state,
                        canvas.width() as f64,
                        canvas.height() as f64,
                    );
                }
            }
        })
    };

    // Mouse down handler for drag start
    let on_mouse_down = {
        let render_state = render_state.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let mut state = render_state.borrow_mut();
            state.is_dragging = true;
            state.drag_start = (e.client_x() as f64, e.client_y() as f64);
            state.pan_start = (state.pan_x, state.pan_y);
        })
    };

    // Mouse move handler for panning
    let on_mouse_move = {
        let canvas_ref = canvas_ref.clone();
        let render_state = render_state.clone();
        let layout = layout.clone();

        Callback::from(move |e: MouseEvent| {
            let mut state = render_state.borrow_mut();
            if !state.is_dragging {
                return;
            }

            let (start_x, start_y) = state.drag_start;
            let (pan_start_x, pan_start_y) = state.pan_start;
            let dx = e.client_x() as f64 - start_x;
            let dy = e.client_y() as f64 - start_y;
            state.pan_x = pan_start_x + dx;
            state.pan_y = pan_start_y + dy;

            // Re-render canvas directly
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                if let Ok(Some(context)) = canvas.get_context("2d") {
                    if let Ok(ctx) = context.dyn_into::<CanvasRenderingContext2d>() {
                        render_canvas(
                            &ctx,
                            &layout,
                            &state,
                            canvas.width() as f64,
                            canvas.height() as f64,
                        );
                    }
                }
            }
        })
    };

    // Mouse up handler for drag end
    let on_mouse_up = {
        let render_state = render_state.clone();

        Callback::from(move |_: MouseEvent| {
            render_state.borrow_mut().is_dragging = false;
        })
    };

    // Mouse leave handler
    let on_mouse_leave = {
        let render_state = render_state.clone();

        Callback::from(move |_: MouseEvent| {
            render_state.borrow_mut().is_dragging = false;
        })
    };

    // Reset view handler
    let on_reset_view = {
        let canvas_ref = canvas_ref.clone();
        let render_state = render_state.clone();
        let layout = layout.clone();

        Callback::from(move |_: MouseEvent| {
            {
                let mut state = render_state.borrow_mut();
                state.zoom = 1.0;
                state.pan_x = 0.0;
                state.pan_y = 0.0;
            }

            // Re-render canvas
            if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                if let Ok(Some(context)) = canvas.get_context("2d") {
                    if let Ok(ctx) = context.dyn_into::<CanvasRenderingContext2d>() {
                        let state = render_state.borrow();
                        render_canvas(
                            &ctx,
                            &layout,
                            &state,
                            canvas.width() as f64,
                            canvas.height() as f64,
                        );
                    }
                }
            }
        })
    };

    // Show appropriate UI based on loading state
    // Only show loading/error states if we haven't loaded successfully before
    if props.room_id.is_empty() {
        return html! {
            <div style="text-align: center; color: #666; padding: 20px;">
                { "Enter a room ID to view topology" }
            </div>
        };
    }

    // First-time loading state (before any successful load)
    if !*has_loaded_once {
        match &*loading_state {
            LoadingState::Idle | LoadingState::Loading => {
                return html! {
                    <div style="text-align: center; color: #667eea; padding: 20px;">
                        { "Loading topology..." }
                    </div>
                };
            }
            LoadingState::Error(err) => {
                return html! {
                    <div style="text-align: center; padding: 20px;">
                        <div style="color: #e91e63; margin-bottom: 10px;">
                            { format!("Error: {}", err) }
                        </div>
                        <button
                            onclick={on_refresh.clone()}
                            style="padding: 6px 12px; background: #667eea; border: none; border-radius: 4px; color: #fff; cursor: pointer;"
                        >{ "Retry" }</button>
                    </div>
                };
            }
            LoadingState::Loaded => {}
        }

        if network_info.is_empty() {
            return html! {
                <div style="text-align: center; color: #666; padding: 20px;">
                    <div style="margin-bottom: 10px;">{ "No peers in this room" }</div>
                    <button
                        onclick={on_refresh}
                        style="padding: 6px 12px; background: #667eea; border: none; border-radius: 4px; color: #fff; cursor: pointer;"
                    >{ "Refresh" }</button>
                </div>
            };
        }
    }

    // Determine if currently loading (for UI indicator)
    let is_loading = matches!(&*loading_state, LoadingState::Loading);

    let cursor_style = {
        let state = render_state.borrow();
        if state.is_dragging {
            "grabbing"
        } else {
            "grab"
        }
    };

    html! {
        <div style="position: relative;">
            // Controls overlay
            <div style="position: absolute; top: 5px; right: 5px; z-index: 10; display: flex; gap: 5px;">
                <button
                    onclick={on_refresh}
                    disabled={is_loading}
                    style="
                        padding: 4px 8px;
                        background: #667eea;
                        border: 1px solid #555;
                        border-radius: 4px;
                        color: #fff;
                        font-size: 10px;
                        cursor: pointer;
                    "
                >{ if is_loading { "Loading..." } else { "Refresh" } }</button>
                <button
                    onclick={on_reset_view}
                    style="
                        padding: 4px 8px;
                        background: #333;
                        border: 1px solid #555;
                        border-radius: 4px;
                        color: #fff;
                        font-size: 10px;
                        cursor: pointer;
                    "
                >{ "Reset View" }</button>
            </div>

            // Player count info
            <div style="position: absolute; top: 5px; left: 80px; z-index: 10; color: #888; font-size: 10px;">
                { format!("{} players", network_info.len()) }
                if is_loading {
                    <span style="margin-left: 8px; color: #667eea;">{ "(updating...)" }</span>
                }
            </div>

            <canvas
                ref={canvas_ref}
                width={canvas_width.to_string()}
                height={canvas_height.to_string()}
                style={format!(
                    "background: #0d0d1a; border-radius: 8px; display: block; margin: 0 auto; cursor: {};",
                    cursor_style
                )}
                onwheel={on_wheel}
                onmousedown={on_mouse_down}
                onmousemove={on_mouse_move}
                onmouseup={on_mouse_up.clone()}
                onmouseleave={on_mouse_leave}
            />
        </div>
    }
}
