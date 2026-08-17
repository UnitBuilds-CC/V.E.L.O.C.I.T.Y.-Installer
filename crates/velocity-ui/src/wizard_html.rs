//! Cross-platform wizard HTML generation.
//!
//! Contains the shared HTML/CSS/JS template used by both the Windows
//! WebView2 wizard (`modern.rs`) and the cross-platform wry wizard
//! (`wry_wizard.rs`). This module has no platform-specific dependencies.

/// Generate the complete HTML content for the wizard.
///
/// This includes all CSS (with dark/light theme variables), the page
/// structure, and the JavaScript communication layer.
///
/// The generated HTML uses `chrome.webview.postMessage()` for IPC,
/// which the wry wizard replaces with a cross-platform shim at runtime.
pub fn generate_wizard_html(theme: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="{theme}">
<head>
<meta charset="utf-8">
<meta http-equiv="X-UA-Compatible" content="IE=edge">
<style>
:root, [data-theme="light"] {{
    --bg: #ffffff;
    --bg-secondary: #f5f5f5;
    --fg: #1a1a1a;
    --fg-secondary: #666666;
    --accent: #0078d4;
    --accent-hover: #106ebe;
    --border: #e0e0e0;
    --sidebar-bg: #0078d4;
    --sidebar-fg: #ffffff;
    --btn-bg: #0078d4;
    --btn-fg: #ffffff;
    --btn-secondary-bg: #e0e0e0;
    --btn-secondary-fg: #1a1a1a;
    --input-bg: #ffffff;
    --input-border: #cccccc;
    --card-bg: #ffffff;
    --card-border: #e0e0e0;
    --danger: #d13438;
    --success: #107c10;
}}
[data-theme="dark"] {{
    --bg: #1e1e1e;
    --bg-secondary: #2d2d2d;
    --fg: #e0e0e0;
    --fg-secondary: #a0a0a0;
    --accent: #4cc2ff;
    --accent-hover: #66cdff;
    --border: #404040;
    --sidebar-bg: #252525;
    --sidebar-fg: #e0e0e0;
    --btn-bg: #4cc2ff;
    --btn-fg: #1e1e1e;
    --btn-secondary-bg: #404040;
    --btn-secondary-fg: #e0e0e0;
    --input-bg: #2d2d2d;
    --input-border: #555555;
    --card-bg: #2d2d2d;
    --card-border: #404040;
    --danger: #f1707b;
    --success: #6ccb5f;
}}
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
    font-family: 'Segoe UI Variable', 'Segoe UI', system-ui, -apple-system, sans-serif;
    background: var(--bg);
    color: var(--fg);
    height: 100vh;
    display: flex;
    overflow: hidden;
    font-size: 14px;
}}
/* Sidebar */
.sidebar {{
    width: 220px;
    min-width: 220px;
    background: var(--sidebar-bg);
    color: var(--sidebar-fg);
    display: flex;
    flex-direction: column;
    padding: 24px 16px;
}}
.sidebar-brand {{
    font-size: 18px;
    font-weight: 600;
    margin-bottom: 4px;
}}
.sidebar-version {{
    font-size: 12px;
    opacity: 0.7;
    margin-bottom: 32px;
}}
.sidebar-steps {{
    list-style: none;
    flex: 1;
}}
.sidebar-steps li {{
    padding: 10px 12px;
    border-radius: 6px;
    margin-bottom: 4px;
    font-size: 13px;
    cursor: default;
    transition: background 0.2s;
    display: flex;
    align-items: center;
    gap: 10px;
}}
.sidebar-steps li .step-num {{
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    font-weight: 600;
    background: rgba(255,255,255,0.15);
    flex-shrink: 0;
}}
.sidebar-steps li.active {{
    background: rgba(255,255,255,0.12);
    font-weight: 600;
}}
.sidebar-steps li.active .step-num {{
    background: rgba(255,255,255,0.25);
}}
.sidebar-steps li.completed .step-num {{
    background: var(--success);
    color: #fff;
}}
/* Main content */
.main {{
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
}}
.content {{
    flex: 1;
    padding: 32px 40px;
    overflow-y: auto;
}}
.content h1 {{
    font-size: 24px;
    font-weight: 600;
    margin-bottom: 8px;
}}
.content p.subtitle {{
    color: var(--fg-secondary);
    margin-bottom: 24px;
    line-height: 1.5;
}}
/* Page transitions */
.page {{
    display: none;
    animation: fadeIn 0.25s ease;
}}
.page.active {{
    display: block;
}}
@keyframes fadeIn {{
    from {{ opacity: 0; transform: translateY(8px); }},
    to {{ opacity: 1; transform: translateY(0); }},
}}
/* Buttons */
.btn-row {{
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 16px 40px;
    border-top: 1px solid var(--border);
    background: var(--bg-secondary);
}}
.btn {{
    padding: 8px 20px;
    border: none;
    border-radius: 4px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
    font-family: inherit;
}}
.btn-primary {{
    background: var(--btn-bg);
    color: var(--btn-fg);
}}
.btn-primary:hover {{ background: var(--accent-hover); }}
.btn-secondary {{
    background: var(--btn-secondary-bg);
    color: var(--btn-secondary-fg);
}}
.btn-secondary:hover {{ opacity: 0.85; }}
.btn-danger {{
    background: var(--danger);
    color: #fff;
}}
/* License box */
.license-box {{
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 16px;
    max-height: 280px;
    overflow-y: auto;
    font-family: 'Cascadia Code', 'Consolas', monospace;
    font-size: 12px;
    line-height: 1.6;
    white-space: pre-wrap;
    color: var(--fg-secondary);
}}
/* Directory picker */
.dir-picker {{
    display: flex;
    gap: 8px;
    margin-top: 12px;
}}
.dir-picker input {{
    flex: 1;
    padding: 8px 12px;
    border: 1px solid var(--input-border);
    border-radius: 4px;
    background: var(--input-bg);
    color: var(--fg);
    font-family: inherit;
    font-size: 13px;
}}
.dir-picker input:focus {{
    outline: none;
    border-color: var(--accent);
}}
/* Component list */
.component-list {{
    list-style: none;
    margin-top: 16px;
}}
.component-item {{
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 16px;
    border: 1px solid var(--card-border);
    border-radius: 6px;
    margin-bottom: 8px;
    background: var(--card-bg);
    transition: border-color 0.15s;
}}
.component-item:hover {{
    border-color: var(--accent);
}}
.component-item input[type="checkbox"] {{
    margin-top: 2px;
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
}}
.component-info {{
    flex: 1;
}}
.component-name {{
    font-weight: 600;
    font-size: 13px;
}}
.component-desc {{
    color: var(--fg-secondary);
    font-size: 12px;
    margin-top: 2px;
}}
.component-size {{
    color: var(--fg-secondary);
    font-size: 12px;
    white-space: nowrap;
    margin-top: 2px;
}}
/* Progress */
.progress-bar {{
    width: 100%;
    height: 6px;
    background: var(--bg-secondary);
    border-radius: 3px;
    overflow: hidden;
    margin: 24px 0 12px;
}}
.progress-fill {{
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.3s ease;
}}
.progress-file {{
    color: var(--fg-secondary);
    font-size: 12px;
    margin-top: 8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}}
.progress-percent {{
    font-size: 32px;
    font-weight: 600;
    color: var(--accent);
    margin-top: 16px;
}}
/* Finish page */
.finish-icon {{
    font-size: 48px;
    margin-bottom: 16px;
}}
.checkbox-row {{
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
}}
.checkbox-row input {{
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
}}
/* Space label */
.space-label {{
    color: var(--fg-secondary);
    font-size: 12px;
    margin-top: 12px;
}}
</style>
</head>
<body>
<div class="sidebar">
    <div class="sidebar-brand" id="sidebar-brand">Velocity Installer</div>
    <div class="sidebar-version" id="sidebar-version">v1.0.0</div>
    <ul class="sidebar-steps" id="sidebar-steps">
        <li data-page="welcome" class="active"><span class="step-num">1</span> Welcome</li>
        <li data-page="license"><span class="step-num">2</span> License</li>
        <li data-page="directory"><span class="step-num">3</span> Directory</li>
        <li data-page="components"><span class="step-num">4</span> Components</li>
        <li data-page="progress"><span class="step-num">5</span> Install</li>
        <li data-page="finish"><span class="step-num">6</span> Complete</li>
    </ul>
</div>
<div class="main">
    <div class="content">
        <!-- Welcome -->
        <div class="page active" id="page-welcome">
            <h1>Welcome</h1>
            <p class="subtitle" id="welcome-text">This wizard will guide you through the installation.</p>
        </div>
        <!-- License -->
        <div class="page" id="page-license">
            <h1>License Agreement</h1>
            <p class="subtitle">Please read the license agreement before continuing.</p>
            <div class="license-box" id="license-content">Loading...</div>
        </div>
        <!-- Directory -->
        <div class="page" id="page-directory">
            <h1>Installation Directory</h1>
            <p class="subtitle">Choose where to install the application.</p>
            <div class="dir-picker">
                <input type="text" id="dir-input" placeholder="Installation path">
                <button class="btn btn-secondary" onclick="browse()">Browse...</button>
            </div>
        </div>
        <!-- Components -->
        <div class="page" id="page-components">
            <h1>Select Components</h1>
            <p class="subtitle">Choose which features to install.</p>
            <ul class="component-list" id="component-list"></ul>
            <div class="space-label" id="space-label"></div>
        </div>
        <!-- Progress -->
        <div class="page" id="page-progress">
            <h1>Installing...</h1>
            <div class="progress-percent" id="progress-percent">0%</div>
            <div class="progress-bar"><div class="progress-fill" id="progress-fill" style="width:0%"></div></div>
            <div class="progress-file" id="progress-file">Preparing...</div>
        </div>
        <!-- Finish -->
        <div class="page" id="page-finish">
            <div class="finish-icon">&#10003;</div>
            <h1>Installation Complete</h1>
            <p class="subtitle" id="finish-text">The application has been installed successfully.</p>
            <div class="checkbox-row">
                <input type="checkbox" id="launch-check" checked>
                <label for="launch-check">Launch application after closing</label>
            </div>
        </div>
    </div>
    <div class="btn-row" id="btn-row">
        <button class="btn btn-secondary" id="btn-cancel" onclick="cancel()">Cancel</button>
        <button class="btn btn-secondary" id="btn-back" onclick="back()" style="display:none">Back</button>
        <button class="btn btn-primary" id="btn-next" onclick="next()">Next</button>
    </div>
</div>
<script>
const PAGE_ORDER = ['welcome','license','directory','components','progress','finish'];
let currentPage = 0;

function navigateTo(page) {{
    const idx = PAGE_ORDER.indexOf(page);
    if (idx < 0) return;
    currentPage = idx;
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    document.getElementById('page-' + page).classList.add('active');
    document.querySelectorAll('.sidebar-steps li').forEach((li, i) => {{
        li.classList.remove('active','completed');
        if (i < idx) li.classList.add('completed');
        if (i === idx) li.classList.add('active');
    }});
    // Button visibility
    document.getElementById('btn-back').style.display = (idx > 0 && idx < 5) ? '' : 'none';
    const nextBtn = document.getElementById('btn-next');
    const cancelBtn = document.getElementById('btn-cancel');
    if (page === 'progress') {{
        nextBtn.style.display = 'none';
        cancelBtn.style.display = 'none';
    }} else if (page === 'finish') {{
        nextBtn.textContent = 'Finish';
        nextBtn.style.display = '';
        nextBtn.onclick = function() {{ finish(); }};
        cancelBtn.style.display = 'none';
    }} else if (page === 'components') {{
        nextBtn.textContent = 'Install';
        nextBtn.onclick = function() {{ install(); }};
    }} else {{
        nextBtn.textContent = 'Next';
        nextBtn.onclick = function() {{ next(); }};
        cancelBtn.style.display = '';
    }}
}}

function next() {{
    if (currentPage < PAGE_ORDER.length - 1) {{
        navigateTo(PAGE_ORDER[currentPage + 1]);
        chrome.webview.postMessage(JSON.stringify({{ type: 'next' }}));
    }}
}}
function back() {{
    if (currentPage > 0) {{
        navigateTo(PAGE_ORDER[currentPage - 1]);
        chrome.webview.postMessage(JSON.stringify({{ type: 'back' }}));
    }}
}}
function cancel() {{
    chrome.webview.postMessage(JSON.stringify({{ type: 'cancel' }}));
}}
function install() {{
    navigateTo('progress');
    chrome.webview.postMessage(JSON.stringify({{ type: 'install' }}));
}}
function finish() {{
    const launch = document.getElementById('launch-check').checked;
    chrome.webview.postMessage(JSON.stringify({{ type: 'finish', data: {{ launch: launch }} }}));
}}
function browse() {{
    chrome.webview.postMessage(JSON.stringify({{ type: 'browse' }}));
}}

// Handle messages from Rust
chrome.webview.addEventListener('message', function(event) {{
    const msg = JSON.parse(event.data);
    if (msg.type === 'state') {{
        const s = msg.data;
        document.getElementById('sidebar-brand').textContent = s.app_name;
        document.getElementById('sidebar-version').textContent = 'v' + s.app_version;
        document.getElementById('welcome-text').textContent =
            'This wizard will install ' + s.app_name + ' v' + s.app_version + ' on your computer.';
        document.getElementById('license-content').textContent = s.license_text || 'No license agreement.';
        document.getElementById('dir-input').value = s.install_dir;
        // Components
        const list = document.getElementById('component-list');
        list.innerHTML = '';
        let totalSize = 0;
        s.components.forEach(function(c) {{
            if (c.selected) totalSize += c.size_mb;
            const li = document.createElement('li');
            li.className = 'component-item';
            li.innerHTML = '<input type="checkbox" id="comp-' + c.id + '" ' +
                (c.selected ? 'checked' : '') + (c.mandatory ? ' disabled' : '') +
                ' onchange="toggleComp(\\'' + c.id + '\\')">' +
                '<div class="component-info"><div class="component-name">' + c.name + '</div>' +
                '<div class="component-desc">' + c.description + '</div></div>' +
                '<div class="component-size">' + c.size_mb.toFixed(1) + ' MB</div>';
            list.appendChild(li);
        }});
        document.getElementById('space-label').textContent =
            'Total space required: ' + totalSize.toFixed(1) + ' MB';
        navigateTo(s.page || 'welcome');
    }} else if (msg.type === 'progress') {{
        document.getElementById('progress-percent').textContent = msg.data.percent + '%';
        document.getElementById('progress-fill').style.width = msg.data.percent + '%';
        document.getElementById('progress-file').textContent = msg.data.file || '';
    }} else if (msg.type === 'set_dir') {{
        document.getElementById('dir-input').value = msg.data;
    }} else if (msg.type === 'navigate') {{
        navigateTo(msg.data);
    }}
}});

function toggleComp(id) {{
    chrome.webview.postMessage(JSON.stringify({{ type: 'toggle_component', data: id }}));
}}

// Notify Rust we're ready
chrome.webview.postMessage(JSON.stringify({{ type: 'ready' }}));
</script>
</body>
</html>"#,
        theme = theme
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_html_light_theme() {
        let html = generate_wizard_html("light");
        assert!(html.contains("data-theme=\"light\""));
        assert!(html.contains("Velocity Installer"));
        assert!(html.contains("chrome.webview.postMessage"));
    }

    #[test]
    fn test_generate_html_dark_theme() {
        let html = generate_wizard_html("dark");
        assert!(html.contains("data-theme=\"dark\""));
    }

    #[test]
    fn test_html_contains_all_pages() {
        let html = generate_wizard_html("light");
        assert!(html.contains("page-welcome"));
        assert!(html.contains("page-license"));
        assert!(html.contains("page-directory"));
        assert!(html.contains("page-components"));
        assert!(html.contains("page-progress"));
        assert!(html.contains("page-finish"));
    }
}
