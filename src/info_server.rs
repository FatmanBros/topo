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
        .api-rest-path {
            fill: #8b949e;
            font-size: 10px;
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
        .link-edge.api-call { stroke: #3fb950; }
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

        /* Hidden state (for node selection filter) */
        .node.hidden { visibility: hidden; pointer-events: none; }
        .link-edge.hidden { visibility: hidden; }
        .api-service-group.hidden { visibility: hidden; pointer-events: none; }
        .cluster.hidden { visibility: hidden; }

        /* Selected node highlight */
        .node.selected rect { stroke-width: 3; filter: drop-shadow(0 0 6px rgba(88, 166, 255, 0.5)); }

        /* Info Panel */
        .info-panel {
            position: fixed;
            top: 20px;
            right: 20px;
            width: 320px;
            max-height: calc(100vh - 40px);
            background: #161b22;
            border: 1px solid #30363d;
            border-radius: 8px;
            z-index: 200;
            display: none;
            flex-direction: column;
            overflow: hidden;
        }
        .info-panel.visible {
            display: flex;
        }
        .info-panel-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 12px 16px;
            border-bottom: 1px solid #30363d;
            background: #21262d;
        }
        .info-panel-title {
            font-size: 14px;
            font-weight: 600;
            color: #f0f6fc;
        }
        .info-panel-close {
            background: none;
            border: none;
            color: #8b949e;
            cursor: pointer;
            font-size: 18px;
            padding: 0;
            line-height: 1;
        }
        .info-panel-close:hover {
            color: #f0f6fc;
        }
        .info-panel-content {
            padding: 16px;
            overflow-y: auto;
            flex: 1;
        }
        .info-section {
            margin-bottom: 16px;
        }
        .info-section:last-child {
            margin-bottom: 0;
        }
        .info-section-title {
            font-size: 11px;
            font-weight: 600;
            color: #8b949e;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            margin-bottom: 6px;
        }
        .info-section-content {
            font-size: 13px;
            color: #c9d1d9;
            line-height: 1.5;
        }
        .info-tag {
            display: inline-block;
            padding: 2px 8px;
            background: #21262d;
            border-radius: 4px;
            font-size: 11px;
            margin-right: 6px;
            margin-bottom: 4px;
        }
        .info-tag.page { color: #58a6ff; border: 1px solid #58a6ff; }
        .info-tag.dynamic { color: #f78166; border: 1px solid #f78166; }
        .info-tag.component { color: #a371f7; border: 1px solid #a371f7; }
        .info-tag.api { color: #3fb950; border: 1px solid #3fb950; }
        .info-file {
            font-family: monospace;
            font-size: 12px;
            color: #8b949e;
            word-break: break-all;
        }
        .info-connections {
            list-style: none;
            padding: 0;
            margin: 0;
        }
        .info-connections li {
            padding: 4px 0;
            font-size: 12px;
            display: flex;
            align-items: center;
            gap: 6px;
        }
        .info-connections .arrow {
            color: #8b949e;
        }
        .info-connections .target {
            color: #58a6ff;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>Page Navigation Graph</h1>
        <p>Scroll to zoom. Drag to pan. Click node to filter connections. Esc to clear.</p>
    </div>

    <div class="info-panel" id="info-panel">
        <div class="info-panel-header">
            <span class="info-panel-title" id="info-panel-title">Node Info</span>
            <button class="info-panel-close" onclick="closeInfoPanel()">&times;</button>
        </div>
        <div class="info-panel-content" id="info-panel-content"></div>
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
        <div class="legend-item">
            <div class="legend-line" style="background: #3fb950;"></div>
            <span>API Call</span>
        </div>
    </div>

    <div class="controls">
        <button onclick="resetZoom()">Reset View</button>
        <button onclick="fitToScreen()">Fit to Screen</button>
    </div>

    <div class="tooltip"></div>
    <svg id="graph"></svg>

    <script>
        let svg, g, zoom, bgGroup;
        let graphData, nodeData = {};
        let selectedNodeId = null;  // Currently selected node for filtering
        let originalPositions = {};  // Store original node positions for restore
        let originalApiPositions = {};  // Store original API service positions
        let originalZoneBounds = {};  // Store original zone bounds
        let pageNodeIds = new Set();  // All page node IDs
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
            bgGroup = svg.append('g').attr('class', 'background');

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
            pageNodeIds = new Set(pageNodes.map(n => n.id));

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
                <marker id="arrowhead-api-call" viewBox="0 -5 10 10" refX="10" refY="0"
                        markerWidth="6" markerHeight="6" orient="auto">
                    <path d="M0,-5L10,0L0,5" fill="#3fb950"/>
                </marker>
            `);

            // Render pages graph (layout only with hierarchy edges)
            render(g, dagreGraph);

            // Store original positions for all nodes
            pageNodes.forEach(n => {
                const node = dagreGraph.node(n.id);
                if (node) {
                    originalPositions[n.id] = { x: node.x, y: node.y };
                }
            });

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

            // Draw API services and track positions (both service and endpoint level)
            const apiNodePositions = {};
            const apiEndpointPositions = {};
            let apiY = 80;
            const serviceWidth = 200;

            data.api_services.forEach((service, idx) => {
                // Calculate height: name + rest_path + endpoints
                const hasRestPath = service.rest_path ? 1 : 0;
                const headerHeight = 28 + hasRestPath * 16;
                const serviceHeight = headerHeight + 12 + service.endpoints.length * 28;

                // Store the center position for connection lines (service level)
                apiNodePositions[service.id] = {
                    x: apiStartX,
                    y: apiY + serviceHeight / 2,
                    width: serviceWidth,
                    height: serviceHeight
                };

                const serviceGroup = g.append('g')
                    .attr('class', 'api-service-group')
                    .attr('data-api-id', service.id)
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
                    .attr('y', 20)
                    .text(service.name);

                // REST base path (if present)
                if (service.rest_path) {
                    serviceGroup.append('text')
                        .attr('class', 'api-rest-path')
                        .attr('x', 10)
                        .attr('y', 36)
                        .text(service.rest_path);
                }

                // Endpoints
                const endpointsStartY = headerHeight + 8;
                service.endpoints.forEach((ep, epIdx) => {
                    const epY = endpointsStartY + epIdx * 28;
                    const endpointId = `${service.id}/${ep.name}`;

                    // Store endpoint position for connection lines
                    apiEndpointPositions[endpointId] = {
                        x: apiStartX,
                        y: apiY + epY + 12, // center of endpoint rect
                        width: serviceWidth,
                        height: 24
                    };

                    const epGroup = serviceGroup.append('g')
                        .attr('class', 'api-endpoint')
                        .attr('data-endpoint-id', endpointId)
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
                        .attr('x', 50)
                        .attr('y', 16)
                        .text(`${ep.path} → ${ep.name}()`);
                });

                apiY += serviceHeight + 20;
            });

            // Store original API positions and zone bounds
            originalApiPositions = { ...apiEndpointPositions, ...apiNodePositions };
            originalZoneBounds = {
                centerLineX,
                apiStartX,
                pageGraphWidth,
                pageGraphHeight: Math.max(pageGraphHeight + 160, height)
            };

            // Draw API call edges (page -> specific endpoint)
            const apiEdges = data.edges.filter(e => e.link_type === 'api-call');
            apiEdges.forEach(e => {
                const sourceNode = dagreGraph.node(e.source);
                // Try endpoint-level first, then fall back to service-level
                const targetEndpoint = apiEndpointPositions[e.target] || apiNodePositions[e.target];

                if (!sourceNode || !targetEndpoint) return;

                // Source right edge -> endpoint left edge
                const sx = sourceNode.x + NODE_WIDTH / 2;
                const sy = sourceNode.y;
                const tx = targetEndpoint.x;
                const ty = targetEndpoint.y;

                const controlOffset = Math.max((tx - sx) * 0.4, 40);
                const pathD = `M${sx},${sy} C${sx + controlOffset},${sy} ${tx - controlOffset},${ty} ${tx},${ty}`;

                edgesGroup.append('path')
                    .attr('class', 'link-edge api-call')
                    .attr('d', pathD)
                    .attr('marker-end', 'url(#arrowhead-api-call)')
                    .attr('data-source', e.source)
                    .attr('data-target', e.target);
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

            // Hover events for page nodes (only when not in selection mode)
            g.selectAll('.node').on('mouseenter', function(event) {
                if (selectedNodeId) return;  // Skip hover effects when node is selected
                const id = d3.select(this).datum();
                highlightConnections(id);
                showTooltip(event, nodeData[id]);
            }).on('mouseleave', function() {
                if (selectedNodeId) return;  // Skip hover effects when node is selected
                clearHighlight();
                hideTooltip();
            }).on('click', function(event) {
                const id = d3.select(this).datum();
                selectNode(id);
                showInfoPanel(nodeData[id]);
                event.stopPropagation();
            });

            // Click on API endpoint to show info
            g.selectAll('.api-endpoint').on('click', function(event) {
                const endpointId = d3.select(this).attr('data-endpoint-id');
                if (endpointId) {
                    // Find the service and endpoint info
                    const parts = endpointId.split('/');
                    const serviceName = parts[1]; // @api/servicename/method -> servicename
                    const methodName = parts[2];
                    const service = graphData.api_services.find(s => s.id.includes(serviceName));
                    if (service) {
                        const endpoint = service.endpoints.find(ep => ep.name === methodName);
                        showApiEndpointInfo(service, endpoint);
                    }
                }
                event.stopPropagation();
            });

            // Click on API service header to show service info
            g.selectAll('.api-service-group').on('click', function(event) {
                const apiId = d3.select(this).attr('data-api-id');
                const service = graphData.api_services.find(s => s.id === apiId);
                if (service) {
                    showApiServiceInfo(service);
                }
                event.stopPropagation();
            });

            // Close info panel and clear selection when clicking on background
            svg.on('click', function() {
                clearSelection();
                closeInfoPanel();
            });

            // Escape key to clear selection
            document.addEventListener('keydown', function(event) {
                if (event.key === 'Escape') {
                    clearSelection();
                    closeInfoPanel();
                }
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

        // Get all nodes connected to the given node (including indirect connections)
        function getAllConnectedNodes(nodeId) {
            const connected = new Set([nodeId]);
            const queue = [nodeId];

            while (queue.length > 0) {
                const current = queue.shift();
                graphData.edges.forEach(e => {
                    // Check source -> target
                    if (e.source === current && !connected.has(e.target)) {
                        // Only add page nodes (not API endpoints)
                        if (nodeData[e.target] || e.target.startsWith('@api/')) {
                            connected.add(e.target);
                            if (nodeData[e.target]) queue.push(e.target);
                        }
                    }
                    // Check target -> source
                    if (e.target === current && !connected.has(e.source)) {
                        if (nodeData[e.source]) {
                            connected.add(e.source);
                            queue.push(e.source);
                        }
                    }
                });
            }

            return connected;
        }

        // Select a node and filter to show only connected nodes with re-layout
        function selectNode(nodeId) {
            // If clicking the same node, clear selection
            if (selectedNodeId === nodeId) {
                clearSelection();
                return;
            }

            selectedNodeId = nodeId;
            const connectedNodes = getAllConnectedNodes(nodeId);

            // Get only page nodes (filter out API endpoints for layout)
            const connectedPageNodes = [...connectedNodes].filter(id => pageNodeIds.has(id));

            // Get connected API service IDs
            const connectedApiServices = new Set();
            connectedNodes.forEach(id => {
                if (id.startsWith('@api/')) {
                    const parts = id.split('/');
                    if (parts.length >= 2) {
                        connectedApiServices.add(parts.slice(0, 2).join('/'));
                    }
                }
            });

            // Create new dagre graph for connected nodes only
            const filteredGraph = new dagreD3.graphlib.Graph()
                .setGraph({
                    rankdir: 'LR',
                    nodesep: 40,
                    ranksep: 100,
                    marginx: 40,
                    marginy: 40
                })
                .setDefaultEdgeLabel(() => ({}));

            // Add connected nodes
            connectedPageNodes.forEach(id => {
                filteredGraph.setNode(id, {
                    width: NODE_WIDTH,
                    height: NODE_HEIGHT
                });
            });

            // Add edges between connected nodes
            graphData.edges.forEach(e => {
                if (connectedPageNodes.includes(e.source) && connectedPageNodes.includes(e.target)) {
                    filteredGraph.setEdge(e.source, e.target);
                }
            });

            // Calculate new layout using dagre-d3's bundled dagre
            dagreD3.dagre.layout(filteredGraph);

            // Animate nodes to new positions
            g.selectAll('.node').each(function(id) {
                const node = d3.select(this);
                if (connectedNodes.has(id)) {
                    const newPos = filteredGraph.node(id);
                    if (newPos) {
                        node.transition()
                            .duration(300)
                            .attr('transform', `translate(${newPos.x}, ${newPos.y})`);
                    }
                    node.attr('display', null).classed('selected', id === nodeId);
                } else {
                    node.attr('display', 'none');
                }
            });

            // Hide clusters
            g.selectAll('.cluster').attr('display', 'none');

            // Update edges
            g.selectAll('.link-edge').each(function() {
                const edge = d3.select(this);
                const source = edge.attr('data-source');
                const target = edge.attr('data-target');

                if (connectedNodes.has(source) && connectedNodes.has(target)) {
                    const sourcePos = filteredGraph.node(source);
                    const targetPos = filteredGraph.node(target);
                    if (sourcePos && targetPos) {
                        const sx = sourcePos.x + NODE_WIDTH / 2;
                        const sy = sourcePos.y;
                        const tx = targetPos.x - NODE_WIDTH / 2;
                        const ty = targetPos.y;
                        const controlOffset = Math.max((tx - sx) * 0.4, 40);
                        const pathD = `M${sx},${sy} C${sx + controlOffset},${sy} ${tx - controlOffset},${ty} ${tx},${ty}`;
                        edge.transition().duration(300).attr('d', pathD);
                    }
                    edge.attr('display', null);
                } else {
                    edge.attr('display', 'none');
                }
            });

            // Calculate bounds for visible nodes
            let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
            connectedPageNodes.forEach(id => {
                const pos = filteredGraph.node(id);
                if (pos) {
                    minX = Math.min(minX, pos.x - NODE_WIDTH / 2);
                    minY = Math.min(minY, pos.y - NODE_HEIGHT / 2);
                    maxX = Math.max(maxX, pos.x + NODE_WIDTH / 2);
                    maxY = Math.max(maxY, pos.y + NODE_HEIGHT / 2);
                }
            });

            // Position API services after page nodes
            const newApiStartX = maxX + 150;
            const newCenterLineX = maxX + 75;
            let newApiY = minY;

            // Hide/show and reposition API services
            g.selectAll('.api-service-group').each(function() {
                const group = d3.select(this);
                const apiId = group.attr('data-api-id');
                if (connectedApiServices.has(apiId)) {
                    group.transition().duration(300)
                        .attr('transform', `translate(${newApiStartX}, ${newApiY})`);
                    // Get height from the rect
                    const rect = group.select('.api-service');
                    const h = parseFloat(rect.attr('height')) || 100;
                    newApiY += h + 20;
                    maxX = Math.max(maxX, newApiStartX + 200);
                    maxY = Math.max(maxY, newApiY);
                    group.attr('display', null);
                } else {
                    group.attr('display', 'none');
                }
            });

            // Update API call edges
            g.selectAll('.link-edge.api-call').each(function() {
                const edge = d3.select(this);
                const source = edge.attr('data-source');
                const target = edge.attr('data-target');

                if (connectedNodes.has(source) && connectedNodes.has(target)) {
                    const sourcePos = filteredGraph.node(source);
                    // Find API service position
                    let targetY = minY;
                    let found = false;
                    let searchY = minY;
                    g.selectAll('.api-service-group').each(function() {
                        const grp = d3.select(this);
                        if (grp.attr('display') !== 'none') {
                            const apiId = grp.attr('data-api-id');
                            if (target.startsWith(apiId)) {
                                targetY = searchY + 50;
                                found = true;
                            }
                            const rect = grp.select('.api-service');
                            const h = parseFloat(rect.attr('height')) || 100;
                            searchY += h + 20;
                        }
                    });

                    if (sourcePos && found) {
                        const sx = sourcePos.x + NODE_WIDTH / 2;
                        const sy = sourcePos.y;
                        const tx = newApiStartX;
                        const ty = targetY;
                        const controlOffset = Math.max((tx - sx) * 0.4, 40);
                        const pathD = `M${sx},${sy} C${sx + controlOffset},${sy} ${tx - controlOffset},${ty} ${tx},${ty}`;
                        edge.transition().duration(300).attr('d', pathD);
                        edge.attr('display', null);
                    } else {
                        edge.attr('display', 'none');
                    }
                } else {
                    edge.attr('display', 'none');
                }
            });

            // Update zone backgrounds
            const padding = 40;
            bgGroup.select('.zone-frontend')
                .transition().duration(300)
                .attr('x', minX - padding)
                .attr('y', minY - padding)
                .attr('width', newCenterLineX - minX + padding)
                .attr('height', maxY - minY + padding * 2);

            bgGroup.select('.zone-backend')
                .transition().duration(300)
                .attr('x', newCenterLineX)
                .attr('y', minY - padding)
                .attr('width', maxX - newCenterLineX + padding)
                .attr('height', maxY - minY + padding * 2);

            bgGroup.select('.center-line')
                .transition().duration(300)
                .attr('x1', newCenterLineX)
                .attr('x2', newCenterLineX)
                .attr('y1', minY - padding)
                .attr('y2', maxY + padding);

            // Update zone labels
            bgGroup.selectAll('.zone-label').each(function(d, i) {
                const label = d3.select(this);
                if (i === 0) {
                    label.transition().duration(300)
                        .attr('x', (minX - padding + newCenterLineX) / 2)
                        .attr('y', minY - padding + 20);
                } else {
                    label.transition().duration(300)
                        .attr('x', newCenterLineX + 100)
                        .attr('y', minY - padding + 20);
                }
            });

            // Fit view to visible content (simultaneous with animations)
            fitToVisibleContent(minX - padding, minY - padding, maxX + padding, maxY + padding);
        }

        // Clear the current selection and restore all nodes to original positions
        function clearSelection() {
            if (!selectedNodeId) return;

            selectedNodeId = null;

            // Restore all nodes to original positions
            g.selectAll('.node').each(function(id) {
                const node = d3.select(this);
                const origPos = originalPositions[id];
                if (origPos) {
                    node.transition()
                        .duration(300)
                        .attr('transform', `translate(${origPos.x}, ${origPos.y})`);
                }
                node.attr('display', null).classed('selected', false);
            });

            // Restore edges to original paths (page-to-page)
            g.selectAll('.link-edge:not(.api-call)').each(function() {
                const edge = d3.select(this);
                const source = edge.attr('data-source');
                const target = edge.attr('data-target');
                const sourcePos = originalPositions[source];
                const targetPos = originalPositions[target];

                if (sourcePos && targetPos && targetPos.x > sourcePos.x) {
                    const sx = sourcePos.x + NODE_WIDTH / 2;
                    const sy = sourcePos.y;
                    const tx = targetPos.x - NODE_WIDTH / 2;
                    const ty = targetPos.y;
                    const controlOffset = Math.max((tx - sx) * 0.4, 40);
                    const pathD = `M${sx},${sy} C${sx + controlOffset},${sy} ${tx - controlOffset},${ty} ${tx},${ty}`;
                    edge.transition().duration(300).attr('d', pathD);
                }
                edge.attr('display', null);
            });

            // Restore API services to original positions
            g.selectAll('.api-service-group').each(function() {
                const group = d3.select(this);
                const apiId = group.attr('data-api-id');
                const origPos = originalApiPositions[apiId];
                if (origPos) {
                    group.transition().duration(300)
                        .attr('transform', `translate(${origPos.x}, ${origPos.y - origPos.height / 2})`);
                }
                group.attr('display', null);
            });

            // Restore API call edges
            g.selectAll('.link-edge.api-call').each(function() {
                const edge = d3.select(this);
                const source = edge.attr('data-source');
                const target = edge.attr('data-target');
                const sourcePos = originalPositions[source];
                const targetPos = originalApiPositions[target];

                if (sourcePos && targetPos) {
                    const sx = sourcePos.x + NODE_WIDTH / 2;
                    const sy = sourcePos.y;
                    const tx = targetPos.x;
                    const ty = targetPos.y;
                    const controlOffset = Math.max((tx - sx) * 0.4, 40);
                    const pathD = `M${sx},${sy} C${sx + controlOffset},${sy} ${tx - controlOffset},${ty} ${tx},${ty}`;
                    edge.transition().duration(300).attr('d', pathD);
                }
                edge.attr('display', null);
            });

            // Restore zone backgrounds
            const zb = originalZoneBounds;
            bgGroup.select('.zone-frontend')
                .transition().duration(300)
                .attr('x', 0)
                .attr('y', 0)
                .attr('width', zb.centerLineX)
                .attr('height', zb.pageGraphHeight);

            bgGroup.select('.zone-backend')
                .transition().duration(300)
                .attr('x', zb.centerLineX)
                .attr('y', 0)
                .attr('width', Math.max(500, window.innerWidth - zb.centerLineX))
                .attr('height', zb.pageGraphHeight);

            bgGroup.select('.center-line')
                .transition().duration(300)
                .attr('x1', zb.centerLineX)
                .attr('x2', zb.centerLineX)
                .attr('y1', 0)
                .attr('y2', zb.pageGraphHeight);

            bgGroup.selectAll('.zone-label').each(function(d, i) {
                const label = d3.select(this);
                if (i === 0) {
                    label.transition().duration(300)
                        .attr('x', zb.centerLineX / 2)
                        .attr('y', 30);
                } else {
                    label.transition().duration(300)
                        .attr('x', zb.centerLineX + 150)
                        .attr('y', 30);
                }
            });

            // Show all clusters
            g.selectAll('.cluster').attr('display', null);

            // Clear any highlight state
            clearHighlight();

            // Fit to original view (simultaneous with animations)
            fitToScreen();
        }

        // Fit view to specific bounds
        function fitToVisibleContent(minX, minY, maxX, maxY) {
            const width = window.innerWidth;
            const height = window.innerHeight;
            const padding = 60;

            const contentWidth = maxX - minX;
            const contentHeight = maxY - minY;

            const scale = Math.min(
                (width - padding * 2) / contentWidth,
                (height - padding * 2) / contentHeight,
                1.5
            );

            const tx = (width - contentWidth * scale) / 2 - minX * scale;
            const ty = (height - contentHeight * scale) / 2 - minY * scale;

            svg.transition().duration(300).call(
                zoom.transform,
                d3.zoomIdentity.translate(tx, ty).scale(scale)
            );
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

        // Info panel functions
        function showInfoPanel(node) {
            if (!node) return;

            const panel = document.getElementById('info-panel');
            const title = document.getElementById('info-panel-title');
            const content = document.getElementById('info-panel-content');

            // Set title
            const displayName = node.doc?.name || node.label;
            title.textContent = displayName;

            // Build content
            let html = '';

            // Type tag
            const typeClass = node.is_dynamic ? 'dynamic' : node.node_type;
            const typeLabel = node.is_dynamic ? 'Dynamic Page' : (node.node_type === 'component' ? 'Component' : 'Page');
            html += `<div class="info-section">
                <span class="info-tag ${typeClass}">${typeLabel}</span>
            </div>`;

            // Route/ID
            html += `<div class="info-section">
                <div class="info-section-title">Route</div>
                <div class="info-section-content">${node.id}</div>
            </div>`;

            // Description (from doc comment)
            if (node.doc?.description) {
                html += `<div class="info-section">
                    <div class="info-section-title">Description</div>
                    <div class="info-section-content">${node.doc.description}</div>
                </div>`;
            }

            // File
            html += `<div class="info-section">
                <div class="info-section-title">File</div>
                <div class="info-file">${node.file}</div>
            </div>`;

            // Author/Version
            if (node.doc?.author || node.doc?.version) {
                html += '<div class="info-section">';
                if (node.doc?.author) {
                    html += `<div class="info-section-title">Author</div>
                        <div class="info-section-content">${node.doc.author}</div>`;
                }
                if (node.doc?.version) {
                    html += `<div class="info-section-title">Version</div>
                        <div class="info-section-content">${node.doc.version}</div>`;
                }
                html += '</div>';
            }

            // Connections
            const outgoing = graphData.edges.filter(e => e.source === node.id);
            const incoming = graphData.edges.filter(e => e.target === node.id);

            if (outgoing.length > 0) {
                html += `<div class="info-section">
                    <div class="info-section-title">Links To</div>
                    <ul class="info-connections">`;
                outgoing.forEach(e => {
                    const linkType = e.link_type === 'api-call' ? '🔌' : '→';
                    html += `<li><span class="arrow">${linkType}</span> <span class="target">${e.target}</span></li>`;
                });
                html += '</ul></div>';
            }

            if (incoming.length > 0) {
                html += `<div class="info-section">
                    <div class="info-section-title">Linked From</div>
                    <ul class="info-connections">`;
                incoming.forEach(e => {
                    html += `<li><span class="arrow">←</span> <span class="target">${e.source}</span></li>`;
                });
                html += '</ul></div>';
            }

            content.innerHTML = html;
            panel.classList.add('visible');

            // Hide legend when panel is open
            document.querySelector('.legend').style.display = 'none';
        }

        function showApiServiceInfo(service) {
            const panel = document.getElementById('info-panel');
            const title = document.getElementById('info-panel-title');
            const content = document.getElementById('info-panel-content');

            title.textContent = service.name;

            let html = `<div class="info-section">
                <span class="info-tag api">API Service</span>
            </div>`;

            if (service.rest_path) {
                html += `<div class="info-section">
                    <div class="info-section-title">Base Path</div>
                    <div class="info-section-content">${service.rest_path}</div>
                </div>`;
            }

            html += `<div class="info-section">
                <div class="info-section-title">File</div>
                <div class="info-file">${service.file}</div>
            </div>`;

            html += `<div class="info-section">
                <div class="info-section-title">Endpoints (${service.endpoints.length})</div>
                <ul class="info-connections">`;
            service.endpoints.forEach(ep => {
                html += `<li><span class="api-method ${ep.method}">${ep.method}</span> ${ep.path} → ${ep.name}()</li>`;
            });
            html += '</ul></div>';

            // Find pages that call this service
            const callers = graphData.edges
                .filter(e => e.link_type === 'api-call' && e.target.startsWith(service.id))
                .map(e => e.source);
            const uniqueCallers = [...new Set(callers)];

            if (uniqueCallers.length > 0) {
                html += `<div class="info-section">
                    <div class="info-section-title">Called From</div>
                    <ul class="info-connections">`;
                uniqueCallers.forEach(caller => {
                    html += `<li><span class="arrow">←</span> <span class="target">${caller}</span></li>`;
                });
                html += '</ul></div>';
            }

            content.innerHTML = html;
            panel.classList.add('visible');
            document.querySelector('.legend').style.display = 'none';
        }

        function showApiEndpointInfo(service, endpoint) {
            const panel = document.getElementById('info-panel');
            const title = document.getElementById('info-panel-title');
            const content = document.getElementById('info-panel-content');

            title.textContent = `${service.name}.${endpoint.name}()`;

            let html = `<div class="info-section">
                <span class="info-tag api">API Endpoint</span>
                <span class="api-method ${endpoint.method}">${endpoint.method}</span>
            </div>`;

            html += `<div class="info-section">
                <div class="info-section-title">Path</div>
                <div class="info-section-content">${service.rest_path || ''}${endpoint.path}</div>
            </div>`;

            html += `<div class="info-section">
                <div class="info-section-title">Service</div>
                <div class="info-section-content">${service.name}</div>
            </div>`;

            // Find pages that call this endpoint
            const endpointId = `${service.id}/${endpoint.name}`;
            const callers = graphData.edges
                .filter(e => e.link_type === 'api-call' && e.target === endpointId)
                .map(e => e.source);

            if (callers.length > 0) {
                html += `<div class="info-section">
                    <div class="info-section-title">Called From</div>
                    <ul class="info-connections">`;
                callers.forEach(caller => {
                    html += `<li><span class="arrow">←</span> <span class="target">${caller}</span></li>`;
                });
                html += '</ul></div>';
            }

            content.innerHTML = html;
            panel.classList.add('visible');
            document.querySelector('.legend').style.display = 'none';
        }

        function closeInfoPanel() {
            document.getElementById('info-panel').classList.remove('visible');
            document.querySelector('.legend').style.display = 'block';
        }
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
                .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
                    .expect("Valid ASCII header")),
            "/api/graph" => Response::from_string(&graph_json)
                .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .expect("Valid ASCII header")),
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
