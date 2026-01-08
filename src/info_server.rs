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

        /* Zone backgrounds */
        .zone-frontend { fill: rgba(88, 166, 255, 0.05); }
        .zone-backend { fill: rgba(163, 113, 247, 0.05); }
        .zone-label {
            font-size: 14px;
            font-weight: 600;
            fill: #8b949e;
        }
        .center-line {
            stroke: #30363d;
            stroke-width: 2;
            stroke-dasharray: 8,4;
        }

        /* Node styles - uniform size */
        .node rect {
            stroke-width: 2;
            cursor: pointer;
            transition: opacity 0.2s;
        }
        .node.page rect { fill: #21262d; stroke: #58a6ff; }
        .node.dynamic rect { fill: #21262d; stroke: #f78166; }
        .node.component rect { fill: #1c2128; stroke: #a371f7; }
        .node text, .node tspan {
            fill: #c9d1d9;
            font-size: 11px;
            pointer-events: none;
        }
        .node .route { fill: #8b949e; font-size: 9px; }

        /* API Service container */
        .api-service {
            fill: #161b22;
            stroke: #238636;
            stroke-width: 2;
            rx: 8;
            ry: 8;
        }
        .api-service-label {
            fill: #3fb950;
            font-size: 12px;
            font-weight: 600;
        }
        .api-endpoint rect {
            fill: #21262d;
            stroke: #238636;
            stroke-width: 1;
        }
        .api-endpoint text {
            fill: #c9d1d9;
            font-size: 10px;
        }
        .api-method {
            font-weight: 600;
            font-size: 9px;
        }
        .api-method.GET { fill: #3fb950; }
        .api-method.POST { fill: #d29922; }
        .api-method.PUT { fill: #58a6ff; }
        .api-method.DELETE { fill: #f85149; }

        /* Cluster (compound graph groups) */
        .cluster rect {
            fill: rgba(88, 166, 255, 0.03);
            stroke: #30363d;
            stroke-width: 1;
            stroke-dasharray: 4,4;
            rx: 8;
            ry: 8;
        }
        .cluster text {
            fill: #8b949e;
            font-size: 11px;
            font-weight: 500;
        }

        /* Edge styles - dagre rendered (hierarchy only, hidden) */
        .edgePath path { display: none; }

        /* Link edge styles - manually drawn */
        .link-edge {
            stroke-width: 2;
            fill: none;
            transition: opacity 0.2s, stroke-width 0.2s;
        }
        .link-edge.declarative { stroke: #58a6ff; }
        .link-edge.programmatic { stroke: #a371f7; stroke-dasharray: 5,5; }
        .link-edge.component-link { stroke: #a371f7; stroke-dasharray: 2,2; }
        .link-edge.faded { opacity: 0.15; }
        .link-edge.highlighted { stroke-width: 3; opacity: 1; }

        /* Legend */
        .legend {
            position: fixed;
            top: 20px;
            right: 20px;
            background: #161b22;
            border: 1px solid #30363d;
            padding: 16px;
            border-radius: 8px;
            font-size: 12px;
            z-index: 100;
        }
        .legend h3 {
            margin-bottom: 12px;
            font-size: 13px;
            color: #f0f6fc;
        }
        .legend-item {
            display: flex;
            align-items: center;
            margin-bottom: 6px;
            gap: 8px;
        }
        .legend-color {
            width: 14px;
            height: 14px;
            border-radius: 3px;
        }
        .legend-line {
            width: 20px;
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
            <span>Page</span>
        </div>
        <div class="legend-item">
            <div class="legend-color" style="background: #21262d; border: 2px solid #f78166;"></div>
            <span>Dynamic Page</span>
        </div>
        <div class="legend-item">
            <div class="legend-color" style="background: #161b22; border: 2px solid #238636;"></div>
            <span>API Service</span>
        </div>
        <div class="legend-item">
            <div class="legend-line" style="background: #58a6ff;"></div>
            <span>Link (href)</span>
        </div>
        <div class="legend-item">
            <div class="legend-line" style="background: repeating-linear-gradient(90deg, #a371f7, #a371f7 4px, transparent 4px, transparent 8px);"></div>
            <span>router.push</span>
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
        const NODE_WIDTH = 120;
        const NODE_HEIGHT = 36;

        fetch('/api/graph')
            .then(r => r.json())
            .then(data => {
                graphData = data;
                initGraph(data);
            });

        function initGraph(data) {
            const width = window.innerWidth;
            const height = window.innerHeight;

            svg = d3.select('#graph')
                .attr('width', width)
                .attr('height', height);

            // Background group (for zones)
            const bgGroup = svg.append('g').attr('class', 'background');

            g = svg.append('g');

            // Create dagre graph for pages with compound graph support
            const dagreGraph = new dagreD3.graphlib.Graph({ compound: true })
                .setGraph({
                    rankdir: 'LR',
                    nodesep: 40,
                    ranksep: 100,
                    marginx: 80,
                    marginy: 80
                })
                .setDefaultEdgeLabel(() => ({}));

            // Store node data for hover
            data.nodes.forEach(n => {
                nodeData[n.id] = n;
            });

            // Filter out component nodes (they are handled separately or not shown)
            const pageNodes = data.nodes.filter(n => n.node_type !== 'component');
            const pageNodeIds = new Set(pageNodes.map(n => n.id));

            // Helper: get parent path
            function getParentPath(path) {
                if (path === '/') return null;
                const segments = path.split('/').filter(s => s);
                if (segments.length <= 1) return '/';
                segments.pop();
                return '/' + segments.join('/');
            }

            // Identify path groups for compound graph
            const groupPrefixes = new Set();
            pageNodes.forEach(n => {
                const parts = n.id.split('/').filter(s => s);
                if (parts.length >= 2) {
                    groupPrefixes.add('/' + parts[0]);
                }
            });

            // Add cluster nodes for each group
            groupPrefixes.forEach(prefix => {
                dagreGraph.setNode(`cluster-${prefix}`, {
                    label: prefix + '/',
                    clusterLabelPos: 'top',
                    style: 'fill: rgba(88, 166, 255, 0.03); stroke: #30363d; stroke-dasharray: 4,4;',
                    paddingTop: 30,
                    paddingBottom: 15,
                    paddingLeft: 15,
                    paddingRight: 15
                });
            });

            // Add page nodes with uniform size
            pageNodes.forEach(n => {
                const isDynamic = n.is_dynamic;

                dagreGraph.setNode(n.id, {
                    label: n.label,
                    width: NODE_WIDTH,
                    height: NODE_HEIGHT,
                    class: isDynamic ? 'dynamic' : 'page',
                    rx: 4,
                    ry: 4,
                    route: n.id
                });

                // Assign node to its cluster (group)
                const parts = n.id.split('/').filter(s => s);
                if (parts.length >= 2) {
                    const groupPrefix = '/' + parts[0];
                    dagreGraph.setParent(n.id, `cluster-${groupPrefix}`);
                }
            });

            // Add ONLY hierarchy edges for layout calculation (parent -> child)
            pageNodes.forEach(n => {
                const parentPath = getParentPath(n.id);
                if (parentPath && pageNodeIds.has(parentPath)) {
                    dagreGraph.setEdge(parentPath, n.id, {
                        class: 'hierarchy',
                        minlen: 1
                    });
                }
            });

            // Store actual link edges for drawing after layout
            const linkEdges = data.edges.filter(e =>
                pageNodeIds.has(e.source) && pageNodeIds.has(e.target)
            );


            // Create renderer
            const render = new dagreD3.render();

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

            // Render pages graph (layout only with hierarchy edges)
            render(g, dagreGraph);

            // Draw actual link edges manually after layout
            // Only forward links (left to right) - browser back handles navigation back
            const edgesGroup = g.append('g').attr('class', 'link-edges');

            linkEdges.forEach((e, i) => {
                const sourceNode = dagreGraph.node(e.source);
                const targetNode = dagreGraph.node(e.target);

                if (!sourceNode || !targetNode) return;

                // Only draw forward links (left to right)
                // Skip backward links - browser back button handles navigation back
                if (targetNode.x <= sourceNode.x) return;

                // Source right edge -> target left edge
                const sx = sourceNode.x + NODE_WIDTH / 2;
                const sy = sourceNode.y;
                const tx = targetNode.x - NODE_WIDTH / 2;
                const ty = targetNode.y;
                const controlOffset = Math.max((tx - sx) * 0.4, 40);
                const pathD = `M${sx},${sy} C${sx + controlOffset},${sy} ${tx - controlOffset},${ty} ${tx},${ty}`;

                const edgeClass = e.link_type || 'declarative';
                const markerId = `arrowhead-${edgeClass}`;

                edgesGroup.append('path')
                    .attr('class', `link-edge ${edgeClass}`)
                    .attr('d', pathD)
                    .attr('marker-end', `url(#${markerId})`)
                    .attr('data-source', e.source)
                    .attr('data-target', e.target);
            });


            // Get page graph bounds
            const graphBounds = dagreGraph.graph();
            const pageGraphWidth = graphBounds.width || 400;
            const pageGraphHeight = graphBounds.height || 400;

            // Calculate API section position
            const apiStartX = pageGraphWidth + 200;
            const centerLineX = pageGraphWidth + 100;

            // Draw zone backgrounds and center line
            bgGroup.append('rect')
                .attr('class', 'zone-frontend')
                .attr('x', 0)
                .attr('y', 0)
                .attr('width', centerLineX)
                .attr('height', Math.max(pageGraphHeight + 160, height));

            bgGroup.append('rect')
                .attr('class', 'zone-backend')
                .attr('x', centerLineX)
                .attr('y', 0)
                .attr('width', Math.max(500, width - centerLineX))
                .attr('height', Math.max(pageGraphHeight + 160, height));

            bgGroup.append('line')
                .attr('class', 'center-line')
                .attr('x1', centerLineX)
                .attr('y1', 0)
                .attr('x2', centerLineX)
                .attr('y2', Math.max(pageGraphHeight + 160, height));

            bgGroup.append('text')
                .attr('class', 'zone-label')
                .attr('x', centerLineX / 2)
                .attr('y', 30)
                .attr('text-anchor', 'middle')
                .text('Frontend (Pages)');

            bgGroup.append('text')
                .attr('class', 'zone-label')
                .attr('x', centerLineX + 150)
                .attr('y', 30)
                .attr('text-anchor', 'middle')
                .text('Backend (API)');

            // Draw API services
            let apiY = 80;
            data.api_services.forEach((service, idx) => {
                const serviceHeight = 50 + service.endpoints.length * 28;
                const serviceWidth = 180;

                const serviceGroup = g.append('g')
                    .attr('class', 'api-service-group')
                    .attr('transform', `translate(${apiStartX}, ${apiY})`);

                // Service container
                serviceGroup.append('rect')
                    .attr('class', 'api-service')
                    .attr('width', serviceWidth)
                    .attr('height', serviceHeight);

                // Service name
                serviceGroup.append('text')
                    .attr('class', 'api-service-label')
                    .attr('x', 10)
                    .attr('y', 22)
                    .text(service.name);

                // Endpoints
                service.endpoints.forEach((ep, epIdx) => {
                    const epY = 40 + epIdx * 28;

                    const epGroup = serviceGroup.append('g')
                        .attr('class', 'api-endpoint')
                        .attr('transform', `translate(8, ${epY})`);

                    epGroup.append('rect')
                        .attr('width', serviceWidth - 16)
                        .attr('height', 24)
                        .attr('rx', 3)
                        .attr('ry', 3);

                    epGroup.append('text')
                        .attr('class', `api-method ${ep.method}`)
                        .attr('x', 6)
                        .attr('y', 16)
                        .text(ep.method);

                    epGroup.append('text')
                        .attr('x', 45)
                        .attr('y', 16)
                        .text(`${ep.path} → ${ep.name}()`);
                });

                apiY += serviceHeight + 20;
            });

            // Add route labels under node labels
            g.selectAll('.node').each(function(id) {
                const node = d3.select(this);
                const data = dagreGraph.node(id);
                if (data && data.route) {
                    node.append('text')
                        .attr('class', 'route')
                        .attr('x', 0)
                        .attr('y', NODE_HEIGHT / 2 - 6)
                        .attr('text-anchor', 'middle')
                        .attr('dominant-baseline', 'hanging')
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
                    bgGroup.attr('transform', event.transform);
                });

            svg.call(zoom);

            // Initial fit
            fitToScreen();

            // Hover events for page nodes
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

            g.selectAll('.link-edge')
                .classed('faded', function() {
                    const source = d3.select(this).attr('data-source');
                    const target = d3.select(this).attr('data-target');
                    return source !== nodeId && target !== nodeId;
                })
                .classed('highlighted', function() {
                    const source = d3.select(this).attr('data-source');
                    const target = d3.select(this).attr('data-target');
                    return source === nodeId || target === nodeId;
                });

            g.selectAll('.node')
                .classed('faded', id => !connectedNodes.has(id));
        }

        function clearHighlight() {
            g.selectAll('.link-edge').classed('faded', false).classed('highlighted', false);
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
            const padding = 80;

            const scale = Math.min(
                (width - padding * 2) / bounds.width,
                (height - padding * 2) / bounds.height,
                1.2
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
    println!("  Found {} pages, {} APIs, {} links",
        graph.nodes.len(),
        graph.api_services.len(),
        graph.edges.len()
    );
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
