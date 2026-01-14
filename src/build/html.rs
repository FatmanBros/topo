//! HTML generation for topo builds

use topo::config::Config;

/// Generate HTML for development builds
pub fn generate_html(config: &Config) -> String {
    let style_config = config.style.clone().unwrap_or_default();
    let tailwind_config = style_config.tailwind.unwrap_or_default();

    let tailwind_script = if tailwind_config.enabled && tailwind_config.cdn {
        if let Some(custom_url) = &tailwind_config.cdn_url {
            format!("    <script src=\"{}\"></script>\n", custom_url)
        } else {
            format!(
                "    <script src=\"https://cdn.tailwindcss.com/{}\"></script>\n",
                tailwind_config.version
            )
        }
    } else if tailwind_config.enabled {
        "    <!-- Tailwind CSS: Configure local build in tailwind.config.js -->\n    <link rel=\"stylesheet\" href=\"./styles.css\">\n".to_string()
    } else {
        String::new()
    };

    let title = config
        .project
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "topo App".to_string());

    let title_script = format!("    <script>window.__TOPO_DEFAULT_TITLE = '{}';</script>\n", title);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" type="image/x-icon" href="/favicon.ico">
    <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
    <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
    <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
    <title>{}</title>
{}{}</head>
<body>
    <div id="app"></div>
    <script type="module" src="/app.js"></script>
    <script>
    // Error Overlay for development
    (function() {{
      const overlay = document.createElement('div');
      overlay.id = 'topo-error-overlay';
      overlay.style.cssText = 'display:none;position:fixed;inset:0;background:rgba(0,0,0,0.85);z-index:99999;padding:32px;overflow:auto;font-family:ui-monospace,monospace';

      function showError(title, message, stack) {{
        overlay.innerHTML = `
          <div style="max-width:900px;margin:0 auto;background:#1a1a1a;border-radius:12px;border:1px solid #333;overflow:hidden">
            <div style="background:#dc2626;color:white;padding:16px 20px;display:flex;justify-content:space-between;align-items:center">
              <span style="font-weight:600;font-size:16px">${{title}}</span>
              <div>
                <button id="topo-copy-btn" style="background:#fff2;border:none;color:white;padding:8px 16px;border-radius:6px;cursor:pointer;margin-right:8px;font-size:13px">Copy</button>
                <button id="topo-close-btn" style="background:#fff2;border:none;color:white;padding:8px 16px;border-radius:6px;cursor:pointer;font-size:13px">✕</button>
              </div>
            </div>
            <div style="padding:20px">
              <div style="color:#f87171;font-size:18px;font-weight:500;margin-bottom:16px;word-break:break-word">${{message}}</div>
              ${{stack ? `<pre style="color:#a1a1aa;font-size:13px;line-height:1.6;margin:0;white-space:pre-wrap;word-break:break-word">${{stack}}</pre>` : ''}}
            </div>
          </div>
        `;
        overlay.style.display = 'block';
        document.getElementById('topo-close-btn').onclick = () => overlay.style.display = 'none';
        document.getElementById('topo-copy-btn').onclick = () => {{
          navigator.clipboard.writeText(message + (stack ? '\\n\\n' + stack : ''));
          document.getElementById('topo-copy-btn').textContent = 'Copied!';
          setTimeout(() => document.getElementById('topo-copy-btn').textContent = 'Copy', 2000);
        }};
      }}

      document.body.appendChild(overlay);

      window.onerror = (msg, src, line, col, err) => {{
        const loc = src ? `${{src}}:${{line}}:${{col}}` : '';
        showError('Runtime Error', msg, err?.stack || loc);
        return false;
      }};

      window.onunhandledrejection = (e) => {{
        showError('Unhandled Promise Rejection', e.reason?.message || String(e.reason), e.reason?.stack);
      }};
    }})();
    </script>
</body>
</html>
"#,
        title, tailwind_script, title_script
    )
}

/// Generate HTML for SSG (production) - relative paths, no dev features
pub fn generate_html_ssg(config: &Config, _js: &str) -> String {
    let title = config
        .project
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "topo App".to_string());

    let base_path = config
        .build
        .as_ref()
        .and_then(|b| b.base_path.clone())
        .unwrap_or_default();

    let base_path_script = if !base_path.is_empty() {
        format!("window.__TOPO_BASE_PATH = '{}';", base_path)
    } else {
        String::new()
    };

    let title_script = format!("window.__TOPO_DEFAULT_TITLE = '{}';", title);

    let config_script = if base_path_script.is_empty() {
        format!("    <script>{}</script>\n", title_script)
    } else {
        format!("    <script>{} {}</script>\n", base_path_script, title_script)
    };

    let asset_prefix = if base_path.is_empty() {
        String::from("/")
    } else {
        format!("{}/", base_path)
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" type="image/x-icon" href="{asset_prefix}favicon.ico">
    <link rel="icon" type="image/png" sizes="32x32" href="{asset_prefix}favicon-32x32.png">
    <link rel="icon" type="image/png" sizes="16x16" href="{asset_prefix}favicon-16x16.png">
    <link rel="apple-touch-icon" sizes="180x180" href="{asset_prefix}apple-touch-icon.png">
    <title>{title}</title>
    <link rel="stylesheet" href="{asset_prefix}styles.css">
</head>
<body>
    <div id="app"></div>
{config_script}    <script type="module" src="{asset_prefix}app.js"></script>
</body>
</html>
"#,
        asset_prefix = asset_prefix,
        title = title,
        config_script = config_script
    )
}

/// Generate HTML for development mode with hot reload
pub fn generate_html_dev(config: &Config, ws_port: u16) -> String {
    let style_config = config.style.clone().unwrap_or_default();
    let tailwind_config = style_config.tailwind.unwrap_or_default();

    let tailwind_script = if tailwind_config.enabled && tailwind_config.cdn {
        if let Some(custom_url) = &tailwind_config.cdn_url {
            format!("    <script src=\"{}\"></script>\n", custom_url)
        } else {
            format!(
                "    <script src=\"https://cdn.tailwindcss.com/{}\"></script>\n",
                tailwind_config.version
            )
        }
    } else if tailwind_config.enabled {
        "    <!-- Tailwind CSS: Configure local build in tailwind.config.js -->\n    <link rel=\"stylesheet\" href=\"./styles.css\">\n".to_string()
    } else {
        String::new()
    };

    let title = config
        .project
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "topo App".to_string());

    let title_script = format!("    <script>window.__TOPO_DEFAULT_TITLE = '{}';</script>\n", title);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" type="image/x-icon" href="/favicon.ico">
    <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
    <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
    <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
    <title>{}</title>
{}{}</head>
<body>
    <div id="app"></div>
    <script type="module" src="/app.js"></script>
    <script>
    // Hot Reload WebSocket
    (function() {{
      let connected = false;
      const ws = new WebSocket('ws://localhost:{}');
      ws.onopen = () => {{
        connected = true;
        console.log('[topo] Hot reload connected');
      }};
      ws.onmessage = (e) => {{
        if (e.data === 'reload') {{
          console.log('[topo] Reloading...');
          location.reload();
        }}
      }};
      ws.onclose = () => {{
        if (connected) {{
          console.log('[topo] Connection lost, attempting reconnect...');
          setTimeout(() => location.reload(), 1000);
        }}
      }};
      ws.onerror = () => {{}};
    }})();

    // Error Overlay for development
    (function() {{
      const overlay = document.createElement('div');
      overlay.id = 'topo-error-overlay';
      overlay.style.cssText = 'display:none;position:fixed;inset:0;background:rgba(0,0,0,0.85);z-index:99999;padding:32px;overflow:auto;font-family:ui-monospace,monospace';

      function showError(title, message, stack) {{
        overlay.innerHTML = `
          <div style="max-width:900px;margin:0 auto;background:#1a1a1a;border-radius:12px;border:1px solid #333;overflow:hidden">
            <div style="background:#dc2626;color:white;padding:16px 20px;display:flex;justify-content:space-between;align-items:center">
              <span style="font-weight:600;font-size:16px">${{title}}</span>
              <div>
                <button id="topo-copy-btn" style="background:#fff2;border:none;color:white;padding:8px 16px;border-radius:6px;cursor:pointer;margin-right:8px;font-size:13px">Copy</button>
                <button id="topo-close-btn" style="background:#fff2;border:none;color:white;padding:8px 16px;border-radius:6px;cursor:pointer;font-size:13px">✕</button>
              </div>
            </div>
            <div style="padding:20px">
              <div style="color:#f87171;font-size:18px;font-weight:500;margin-bottom:16px;word-break:break-word">${{message}}</div>
              ${{stack ? `<pre style="color:#a1a1aa;font-size:13px;line-height:1.6;margin:0;white-space:pre-wrap;word-break:break-word">${{stack}}</pre>` : ''}}
            </div>
          </div>
        `;
        overlay.style.display = 'block';
        document.getElementById('topo-close-btn').onclick = () => overlay.style.display = 'none';
        document.getElementById('topo-copy-btn').onclick = () => {{
          navigator.clipboard.writeText(message + (stack ? '\\n\\n' + stack : ''));
          document.getElementById('topo-copy-btn').textContent = 'Copied!';
          setTimeout(() => document.getElementById('topo-copy-btn').textContent = 'Copy', 2000);
        }};
      }}

      document.body.appendChild(overlay);

      window.onerror = (msg, src, line, col, err) => {{
        const loc = src ? `${{src}}:${{line}}:${{col}}` : '';
        showError('Runtime Error', msg, err?.stack || loc);
        return false;
      }};

      window.onunhandledrejection = (e) => {{
        showError('Unhandled Promise Rejection', e.reason?.message || String(e.reason), e.reason?.stack);
      }};
    }})();
    </script>
</body>
</html>
"#,
        title, tailwind_script, title_script, ws_port
    )
}
