//! HTTP server for the page navigation graph visualizer

use crate::link_analyzer::LinkAnalyzer;
use anyhow::Result;
use tiny_http::{Header, Response, Server};

/// HTML template for the visualizer
const VISUALIZER_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>topo - Page Navigation Graph</title>
    <script src="https://d3js.org/d3.v7.min.js"></script>
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
        .node.page.dynamic rect { stroke: #f78166; }
        .node.component rect { fill: #1c2128; stroke: #a371f7; rx: 8; ry: 8; }
        .node text {
            fill: #c9d1d9;
            font-size: 12px;
            pointer-events: none;
        }
        .node .route { fill: #8b949e; font-size: 10px; }
        .node .component-icon {
            fill: #a371f7;
            font-size: 10px;
            cursor: pointer;
        }

        /* Link styles */
        .link {
            fill: none;
            stroke-width: 2;
            transition: opacity 0.2s, stroke-width 0.2s;
        }
        .link.declarative { stroke: #58a6ff; }
        .link.programmatic { stroke: #a371f7; stroke-dasharray: 5,5; }
        .link.component-link { stroke: #a371f7; stroke-dasharray: 2,2; }
        .link.faded { opacity: 0.15; }
        .link.highlighted { stroke-width: 3; opacity: 1; }

        /* Arrowhead */
        .arrowhead { fill: #58a6ff; }
        .arrowhead.programmatic { fill: #a371f7; }
        .arrowhead.component-link { fill: #a371f7; }

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

        /* Faded state for nodes */
        .node.faded rect { opacity: 0.3; }
        .node.faded text { opacity: 0.3; }
    </style>
</head>
<body>
    <div class="header">
        <h1>Page Navigation Graph</h1>
        <p>Drag nodes to rearrange. Hover to highlight connections.</p>
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
        <button onclick="toggleComponents()">Toggle Components</button>
    </div>

    <div class="tooltip"></div>
    <svg id="graph"></svg>

    <script>
        let showComponents = true;
        let simulation;
        let svg, g;
        let graphData;

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

            // Define arrowhead markers
            const defs = svg.append('defs');

            ['declarative', 'programmatic', 'component-link'].forEach(type => {
                defs.append('marker')
                    .attr('id', `arrowhead-${type}`)
                    .attr('viewBox', '0 -5 10 10')
                    .attr('refX', 20)
                    .attr('refY', 0)
                    .attr('markerWidth', 6)
                    .attr('markerHeight', 6)
                    .attr('orient', 'auto')
                    .append('path')
                    .attr('d', 'M0,-5L10,0L0,5')
                    .attr('class', `arrowhead ${type}`);
            });

            // Create zoom behavior
            const zoom = d3.zoom()
                .scaleExtent([0.1, 4])
                .on('zoom', (event) => {
                    g.attr('transform', event.transform);
                });

            svg.call(zoom);

            // Create container group
            g = svg.append('g');

            // Process nodes - separate pages and components
            const nodes = data.nodes.map(n => ({
                ...n,
                nodeType: n.id.startsWith('@') ? 'component' : 'page'
            }));

            // Create edges with proper source/target references
            const nodeMap = new Map(nodes.map(n => [n.id, n]));
            const edges = data.edges.filter(e => nodeMap.has(e.source) && nodeMap.has(e.target))
                .map(e => ({
                    ...e,
                    source: e.source,
                    target: e.target
                }));

            // Force simulation
            simulation = d3.forceSimulation(nodes)
                .force('link', d3.forceLink(edges).id(d => d.id).distance(180))
                .force('charge', d3.forceManyBody().strength(-400))
                .force('center', d3.forceCenter(width / 2, height / 2))
                .force('collision', d3.forceCollide().radius(80));

            // Draw links
            const link = g.append('g')
                .attr('class', 'links')
                .selectAll('path')
                .data(edges)
                .join('path')
                .attr('class', d => `link ${d.link_type}`)
                .attr('marker-end', d => `url(#arrowhead-${d.link_type})`);

            // Draw nodes
            const node = g.append('g')
                .attr('class', 'nodes')
                .selectAll('g')
                .data(nodes)
                .join('g')
                .attr('class', d => {
                    let cls = `node ${d.nodeType}`;
                    if (d.is_dynamic) cls += ' dynamic';
                    return cls;
                })
                .call(d3.drag()
                    .on('start', dragstarted)
                    .on('drag', dragged)
                    .on('end', dragended));

            // Node rectangle
            node.append('rect')
                .attr('width', d => d.nodeType === 'component' ? 120 : 140)
                .attr('height', d => d.nodeType === 'component' ? 40 : 50)
                .attr('x', d => d.nodeType === 'component' ? -60 : -70)
                .attr('y', d => d.nodeType === 'component' ? -20 : -25)
                .attr('rx', d => d.nodeType === 'component' ? 8 : 4)
                .attr('ry', d => d.nodeType === 'component' ? 8 : 4);

            // Node label
            node.append('text')
                .attr('text-anchor', 'middle')
                .attr('dy', d => d.nodeType === 'component' ? 4 : -2)
                .text(d => d.label);

            // Route path for pages
            node.filter(d => d.nodeType === 'page')
                .append('text')
                .attr('class', 'route')
                .attr('text-anchor', 'middle')
                .attr('dy', 14)
                .text(d => d.id);

            // Component icon for pages that use shared components
            const pagesWithComponents = new Set(
                edges.filter(e => e.source.startsWith && !e.source.startsWith('@') && e.target.startsWith('@'))
                    .map(e => e.source)
            );

            node.filter(d => d.nodeType === 'page' && pagesWithComponents.has(d.id))
                .append('text')
                .attr('class', 'component-icon')
                .attr('x', 55)
                .attr('y', -10)
                .text('◆')
                .on('mouseenter', function(event, d) {
                    highlightComponentConnections(d);
                })
                .on('mouseleave', clearHighlight);

            // Hover effects
            node.on('mouseenter', function(event, d) {
                highlightConnections(d);
                showTooltip(event, d);
            })
            .on('mouseleave', function() {
                clearHighlight();
                hideTooltip();
            });

            // Update positions on tick
            simulation.on('tick', () => {
                link.attr('d', d => {
                    const dx = d.target.x - d.source.x;
                    const dy = d.target.y - d.source.y;
                    return `M${d.source.x},${d.source.y}L${d.target.x},${d.target.y}`;
                });

                node.attr('transform', d => `translate(${d.x},${d.y})`);
            });

            // Store references
            window.nodeSelection = node;
            window.linkSelection = link;
            window.zoomBehavior = zoom;
        }

        function highlightConnections(d) {
            const connectedNodes = new Set([d.id]);

            window.linkSelection.each(function(link) {
                const sourceId = typeof link.source === 'object' ? link.source.id : link.source;
                const targetId = typeof link.target === 'object' ? link.target.id : link.target;

                if (sourceId === d.id) connectedNodes.add(targetId);
                if (targetId === d.id) connectedNodes.add(sourceId);
            });

            window.linkSelection
                .classed('faded', link => {
                    const sourceId = typeof link.source === 'object' ? link.source.id : link.source;
                    const targetId = typeof link.target === 'object' ? link.target.id : link.target;
                    return sourceId !== d.id && targetId !== d.id;
                })
                .classed('highlighted', link => {
                    const sourceId = typeof link.source === 'object' ? link.source.id : link.source;
                    const targetId = typeof link.target === 'object' ? link.target.id : link.target;
                    return sourceId === d.id || targetId === d.id;
                });

            window.nodeSelection
                .classed('faded', n => !connectedNodes.has(n.id));
        }

        function highlightComponentConnections(d) {
            // Highlight only component connections for this page
            const connectedComponents = new Set();

            window.linkSelection.each(function(link) {
                const sourceId = typeof link.source === 'object' ? link.source.id : link.source;
                const targetId = typeof link.target === 'object' ? link.target.id : link.target;

                if (sourceId === d.id && targetId.startsWith('@')) {
                    connectedComponents.add(targetId);
                }
            });

            window.linkSelection
                .classed('faded', link => {
                    const sourceId = typeof link.source === 'object' ? link.source.id : link.source;
                    const targetId = typeof link.target === 'object' ? link.target.id : link.target;
                    return !(sourceId === d.id && connectedComponents.has(targetId));
                })
                .classed('highlighted', link => {
                    const sourceId = typeof link.source === 'object' ? link.source.id : link.source;
                    const targetId = typeof link.target === 'object' ? link.target.id : link.target;
                    return sourceId === d.id && connectedComponents.has(targetId);
                });

            window.nodeSelection
                .classed('faded', n => n.id !== d.id && !connectedComponents.has(n.id));
        }

        function clearHighlight() {
            window.linkSelection
                .classed('faded', false)
                .classed('highlighted', false);
            window.nodeSelection
                .classed('faded', false);
        }

        function showTooltip(event, d) {
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

        function dragstarted(event, d) {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            d.fx = d.x;
            d.fy = d.y;
        }

        function dragged(event, d) {
            d.fx = event.x;
            d.fy = event.y;
        }

        function dragended(event, d) {
            if (!event.active) simulation.alphaTarget(0);
            d.fx = null;
            d.fy = null;
        }

        function resetZoom() {
            svg.transition().duration(750).call(
                window.zoomBehavior.transform,
                d3.zoomIdentity
            );
        }

        function toggleComponents() {
            showComponents = !showComponents;
            window.nodeSelection
                .filter(d => d.nodeType === 'component')
                .style('display', showComponents ? null : 'none');
            window.linkSelection
                .filter(d => {
                    const targetId = typeof d.target === 'object' ? d.target.id : d.target;
                    return targetId.startsWith('@');
                })
                .style('display', showComponents ? null : 'none');
        }

        // Handle window resize
        window.addEventListener('resize', () => {
            const width = window.innerWidth;
            const height = window.innerHeight;
            svg.attr('width', width).attr('height', height);
            simulation.force('center', d3.forceCenter(width / 2, height / 2));
            simulation.alpha(0.3).restart();
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
