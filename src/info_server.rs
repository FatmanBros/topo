//! HTTP server for the page navigation graph visualizer

use crate::link_analyzer::LinkAnalyzer;
use anyhow::Result;
use tiny_http::{Header, Response, Server};

/// HTML template for the visualizer using dagre-d3
const VISUALIZER_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>topo - Page Navigation Graph</title>
    <script src="https://d3js.org/d3.v7.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/dagre-d3@0.6.4/dist/dagre-d3.min.js"></script>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0d1117;
            color: #c9d1d9;
            overflow: hidden;
        }
        #graph { width: 100vw; height: 100vh; }

        /* Node styles */
        .node rect {
            stroke-width: 2;
            cursor: pointer;
            transition: opacity 0.2s;
        }
        .node.page rect { fill: #21262d; stroke: #58a6ff; }
        .node.dynamic rect { fill: #21262d; stroke: #f78166; }
        .node.component rect { fill: #1c2128; stroke: #a371f7; rx: 12; ry: 12; }
        .node text, .node tspan {
            fill: #c9d1d9;
            font-size: 12px;
            pointer-events: none;
        }
        .node .route { fill: #8b949e; font-size: 10px; }

        /* Edge styles */
        .edgePath path {
            stroke-width: 2;
            fill: none;
            transition: opacity 0.2s, stroke-width 0.2s;
        }
        .edgePath.declarative path { stroke: #58a6ff; }
        .edgePath.programmatic path { stroke: #a371f7; stroke-dasharray: 5,5; }
        .edgePath.component-link path { stroke: #a371f7; stroke-dasharray: 2,2; }
        .edgePath.faded path { opacity: 0.15; }
        .edgePath.highlighted path { stroke-width: 3; opacity: 1; }

        /* Arrowhead */
        .edgePath marker path { fill: #58a6ff; }
        .edgePath.programmatic marker path { fill: #a371f7; }
        .edgePath.component-link marker path { fill: #a371f7; }

        /* Legend */
        .legend {
            position: fixed;
            top: 20px;
            right: 20px;
            background: #161b22;
            border: 1px solid #30363d;
            padding: 16px;
            border-radius: 8px;
            font-size: 13px;
            z-index: 100;
        }
        .legend h3 {
            margin-bottom: 12px;
            font-size: 14px;
            color: #f0f6fc;
        }
        .legend-item {
            display: flex;
            align-items: center;
            margin-bottom: 8px;
            gap: 8px;
        }
        .legend-color {
            width: 16px;
            height: 16px;
            border-radius: 3px;
        }
        .legend-line {
            width: 24px;
            height: 2px;
        }

        /* Tooltip */
        .tooltip {
            position: absolute;
            background: #161b22;
            border: 1px solid #30363d;
            padding: 8px 12px;
            border-radius: 6px;
            font-size: 12px;
            pointer-events: none;
            opacity: 0;
            transition: opacity 0.2s;
            z-index: 1000;
            max-width: 300px;
        }
        .tooltip.visible { opacity: 1; }
        .tooltip .file { color: #8b949e; font-size: 11px; }

        /* Header */
        .header {
            position: fixed;
            top: 20px;
            left: 20px;
            z-index: 100;
        }
        .header h1 {
            font-size: 18px;
            font-weight: 600;
            color: #f0f6fc;
        }
        .header p {
            font-size: 12px;
            color: #8b949e;
            margin-top: 4px;
        }

        /* Controls */
        .controls {
            position: fixed;
            bottom: 20px;
            left: 20px;
            display: flex;
            gap: 8px;
            z-index: 100;
        }
        .controls button {
            background: #21262d;
            border: 1px solid #30363d;
            color: #c9d1d9;
            padding: 8px 12px;
            border-radius: 6px;
            cursor: pointer;
            font-size: 12px;
        }
        .controls button:hover {
            background: #30363d;
        }

        /* Faded state */
        .node.faded rect { opacity: 0.3; }
        .node.faded text { opacity: 0.3; }
    </style>
</head>
<body>
    <div class="header">
        <h1>Page Navigation Graph</h1>
        <p>Scroll to zoom. Drag to pan. Hover to highlight.</p>
    </div>

    <div class="legend">
        <h3>Legend</h3>
        <div class="legend-item">
            <div class="legend-color" style="background: #21262d; border: 2px solid #58a6ff;"></div>
            <span>Static Page</span>
        </div>
        <div class="legend-item">
            <div class="legend-color" style="background: #21262d; border: 2px solid #f78166;"></div>
            <span>Dynamic Page ([id])</span>
        </div>
        <div class="legend-item">
            <div class="legend-color" style="background: #1c2128; border: 2px solid #a371f7; border-radius: 8px;"></div>
            <span>Shared Component</span>
        </div>
        <div class="legend-item">
            <div class="legend-line" style="background: #58a6ff;"></div>
            <span>Declarative Link (href)</span>
        </div>
        <div class="legend-item">
            <div class="legend-line" style="background: #a371f7; background: repeating-linear-gradient(90deg, #a371f7, #a371f7 5px, transparent 5px, transparent 10px);"></div>
            <span>Programmatic (router.push)</span>
        </div>
    </div>

    <div class="controls">
        <button onclick="resetZoom()">Reset View</button>
        <button onclick="fitToScreen()">Fit to Screen</button>
    </div>

    <div class="tooltip"></div>
    <svg id="graph"></svg>

    <script>
        let svg, g, zoom;
        let graphData, nodeData = {};

        fetch('/api/graph')
            .then(r => r.json())
            .then(data => {
                graphData = data;
                initGraph(data);
            });

        function initGraph(data) {
            const width = window.innerWidth;
            const height = window.innerHeight;

            // Create dagre graph
            const dagreGraph = new dagreD3.graphlib.Graph()
                .setGraph({
                    rankdir: 'LR',      // Left to Right
                    nodesep: 50,        // Vertical spacing
                    ranksep: 120,       // Horizontal spacing
                    marginx: 50,
                    marginy: 50
                })
                .setDefaultEdgeLabel(() => ({}));

            // Store node data for hover
            data.nodes.forEach(n => {
                nodeData[n.id] = n;
            });

            // Add nodes
            data.nodes.forEach(n => {
                const isComponent = n.id.startsWith('@');
                const isDynamic = n.is_dynamic;
                const label = n.label;
                const route = isComponent ? '' : n.id;

                dagreGraph.setNode(n.id, {
                    label: label,
                    labelType: 'html',
                    class: isComponent ? 'component' : (isDynamic ? 'dynamic' : 'page'),
                    rx: isComponent ? 12 : 4,
                    ry: isComponent ? 12 : 4,
                    padding: 12,
                    route: route
                });
            });

            // Add edges
            data.edges.forEach(e => {
                dagreGraph.setEdge(e.source, e.target, {
                    class: e.link_type,
                    curve: d3.curveBasis,
                    arrowhead: 'vee'
                });
            });

            // Create renderer
            const render = new dagreD3.render();

            svg = d3.select('#graph')
                .attr('width', width)
                .attr('height', height);

            g = svg.append('g');

            // Define arrowheads
            svg.append('defs').html(`
                <marker id="arrowhead-declarative" viewBox="0 -5 10 10" refX="10" refY="0"
                        markerWidth="6" markerHeight="6" orient="auto">
                    <path d="M0,-5L10,0L0,5" fill="#58a6ff"/>
                </marker>
                <marker id="arrowhead-programmatic" viewBox="0 -5 10 10" refX="10" refY="0"
                        markerWidth="6" markerHeight="6" orient="auto">
                    <path d="M0,-5L10,0L0,5" fill="#a371f7"/>
                </marker>
                <marker id="arrowhead-component-link" viewBox="0 -5 10 10" refX="10" refY="0"
                        markerWidth="6" markerHeight="6" orient="auto">
                    <path d="M0,-5L10,0L0,5" fill="#a371f7"/>
                </marker>
            `);

            // Render the graph
            render(g, dagreGraph);

            // Add route labels under node labels
            g.selectAll('.node').each(function(id) {
                const node = d3.select(this);
                const data = dagreGraph.node(id);
                if (data.route) {
                    const bbox = node.select('rect').node().getBBox();
                    node.append('text')
                        .attr('class', 'route')
                        .attr('x', 0)
                        .attr('y', bbox.height / 2 - 5)
                        .attr('text-anchor', 'middle')
                        .text(data.route);
                }
            });

            // Set arrowheads
            g.selectAll('.edgePath').each(function() {
                const edge = d3.select(this);
                const cls = edge.attr('class');
                if (cls.includes('declarative')) {
                    edge.select('path').attr('marker-end', 'url(#arrowhead-declarative)');
                } else if (cls.includes('programmatic')) {
                    edge.select('path').attr('marker-end', 'url(#arrowhead-programmatic)');
                } else if (cls.includes('component-link')) {
                    edge.select('path').attr('marker-end', 'url(#arrowhead-component-link)');
                }
            });

            // Setup zoom
            zoom = d3.zoom()
                .scaleExtent([0.1, 4])
                .on('zoom', (event) => {
                    g.attr('transform', event.transform);
                });

            svg.call(zoom);

            // Initial fit
            fitToScreen();

            // Hover events
            g.selectAll('.node').on('mouseenter', function(event) {
                const id = d3.select(this).datum();
                highlightConnections(id);
                showTooltip(event, nodeData[id]);
            }).on('mouseleave', function() {
                clearHighlight();
                hideTooltip();
            });
        }

        function highlightConnections(nodeId) {
            const connectedNodes = new Set([nodeId]);

            graphData.edges.forEach(e => {
                if (e.source === nodeId) connectedNodes.add(e.target);
                if (e.target === nodeId) connectedNodes.add(e.source);
            });

            g.selectAll('.edgePath').each(function() {
                const edge = d3.select(this);
                const path = edge.select('path');
                // Get edge data from class or data attribute
                const isConnected = graphData.edges.some(e =>
                    (e.source === nodeId || e.target === nodeId)
                );
            });

            g.selectAll('.edgePath')
                .classed('faded', function() {
                    const edgeId = d3.select(this).datum();
                    const edge = graphData.edges.find(e =>
                        dagreD3.graphlib && edgeId && edgeId.v === e.source && edgeId.w === e.target
                    );
                    return !graphData.edges.some(e =>
                        (e.source === nodeId && e.target === (edgeId?.w)) ||
                        (e.target === nodeId && e.source === (edgeId?.v))
                    );
                })
                .classed('highlighted', function() {
                    const edgeId = d3.select(this).datum();
                    return graphData.edges.some(e =>
                        (e.source === nodeId && e.target === (edgeId?.w)) ||
                        (e.target === nodeId && e.source === (edgeId?.v))
                    );
                });

            g.selectAll('.node')
                .classed('faded', id => !connectedNodes.has(id));
        }

        function clearHighlight() {
            g.selectAll('.edgePath').classed('faded', false).classed('highlighted', false);
            g.selectAll('.node').classed('faded', false);
        }

        function showTooltip(event, d) {
            if (!d) return;
            const tooltip = document.querySelector('.tooltip');
            tooltip.innerHTML = `
                <strong>${d.label}</strong><br>
                <span class="file">${d.file}</span>
            `;
            tooltip.style.left = (event.pageX + 10) + 'px';
            tooltip.style.top = (event.pageY + 10) + 'px';
            tooltip.classList.add('visible');
        }

        function hideTooltip() {
            document.querySelector('.tooltip').classList.remove('visible');
        }

        function resetZoom() {
            svg.transition().duration(500).call(
                zoom.transform,
                d3.zoomIdentity
            );
        }

        function fitToScreen() {
            const bounds = g.node().getBBox();
            const width = window.innerWidth;
            const height = window.innerHeight;
            const padding = 60;

            const scale = Math.min(
                (width - padding * 2) / bounds.width,
                (height - padding * 2) / bounds.height,
                1.5
            );

            const tx = (width - bounds.width * scale) / 2 - bounds.x * scale;
            const ty = (height - bounds.height * scale) / 2 - bounds.y * scale;

            svg.transition().duration(500).call(
                zoom.transform,
                d3.zoomIdentity.translate(tx, ty).scale(scale)
            );
        }

        window.addEventListener('resize', () => {
            svg.attr('width', window.innerWidth).attr('height', window.innerHeight);
            fitToScreen();
        });
    </script>
</body>
</html>
"##;

/// Start the info server
pub fn start_info_server(port: u16, no_open: bool) -> Result<()> {
    // Analyze links and build graph
    let analyzer = LinkAnalyzer::new()?;
    let graph = analyzer.build_graph()?;
    let graph_json = serde_json::to_string(&graph)?;

    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr).map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("Address already in use") || err_str.contains("os error 98") {
            anyhow::anyhow!(
                "Port {} is already in use.\n\n\
                 Try one of the following:\n\
                 • Stop the other process using port {}\n\
                 • Use a different port: topo info web --port {}\n\
                 • Kill the process: lsof -ti:{} | xargs kill -9",
                port, port, port + 1, port
            )
        } else {
            anyhow::anyhow!("Failed to start server: {}", e)
        }
    })?;

    println!("\n  📊 Page Navigation Graph");
    println!("  ────────────────────────────────────────────────────");
    println!("  Local:   http://localhost:{}", port);
    println!();
    println!("  Found {} pages, {} links", graph.nodes.len(), graph.edges.len());
    println!();
    println!("  Press Ctrl+C to stop\n");

    // Open browser
    if !no_open {
        let url = format!("http://localhost:{}", port);
        let _ = open_in_browser(&url);
    }

    // Handle requests
    for request in server.incoming_requests() {
        let response = match request.url() {
            "/" => Response::from_string(VISUALIZER_HTML)
                .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap()),
            "/api/graph" => Response::from_string(&graph_json)
                .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()),
            _ => Response::from_string("404 Not Found").with_status_code(404),
        };
        let _ = request.respond(response);
    }

    Ok(())
}

/// Open URL in the default browser
fn open_in_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }
    Ok(())
}
