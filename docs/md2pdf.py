#!/usr/bin/env python3
"""Convert markdown to PDF using Python markdown + weasyprint or fallback to cupsfilter."""
import sys
import os
import re
import tempfile
import subprocess

def markdown_to_html(md_path, css_path=None):
    """Convert markdown to styled HTML."""
    import markdown

    with open(md_path, 'r', encoding='utf-8') as f:
        content = f.read()

    extensions = ['tables', 'fenced_code', 'codehilite', 'toc', 'attr_list', 'md_in_html']
    md = markdown.Markdown(extensions=extensions)
    body = md.convert(content)

    # Generate CSS
    css = """
    @page {
        size: A4;
        margin: 2cm 1.5cm;
        @bottom-center {
            content: "Page " counter(page) " / " counter(pages);
            font-size: 9pt;
            color: #666;
        }
    }

    * { box-sizing: border-box; }

    body {
        font-family: "PingFang SC", "Microsoft YaHei", "Helvetica Neue", Arial, sans-serif;
        font-size: 11pt;
        line-height: 1.6;
        color: #1a1a2e;
        margin: 0;
        padding: 0;
    }

    h1 {
        font-size: 22pt;
        color: #1a1a2e;
        border-bottom: 3px solid #e94560;
        padding-bottom: 8px;
        margin-top: 36pt;
        margin-bottom: 16pt;
        page-break-before: auto;
    }

    h2 {
        font-size: 16pt;
        color: #16213e;
        border-bottom: 1px solid #ddd;
        padding-bottom: 4px;
        margin-top: 24pt;
        margin-bottom: 12pt;
    }

    h3 {
        font-size: 13pt;
        color: #0f3460;
        margin-top: 18pt;
        margin-bottom: 8pt;
    }

    p {
        margin: 8pt 0;
        text-align: justify;
    }

    code {
        font-family: "SF Mono", "Fira Code", "JetBrains Mono", "Courier New", monospace;
        background: #f8f8f8;
        padding: 2px 6px;
        border-radius: 3px;
        font-size: 9pt;
        color: #e94560;
    }

    pre {
        background: #f8f8f8;
        border: 1px solid #e0e0e0;
        border-left: 4px solid #e94560;
        padding: 12pt;
        border-radius: 4px;
        overflow: hidden;
        page-break-inside: avoid;
    }

    pre code {
        background: none;
        padding: 0;
        color: #1a1a2e;
        font-size: 8.5pt;
        line-height: 1.4;
    }

    table {
        width: 100%;
        border-collapse: collapse;
        margin: 12pt 0;
        page-break-inside: avoid;
        font-size: 9.5pt;
    }

    th {
        background: #16213e;
        color: white;
        padding: 8pt 6pt;
        text-align: left;
        font-weight: 600;
    }

    td {
        padding: 6pt;
        border-bottom: 1px solid #eee;
    }

    tr:nth-child(even) { background: #fafafa; }

    blockquote {
        border-left: 4px solid #e94560;
        margin: 12pt 0;
        padding: 8pt 16pt;
        background: #fff5f7;
        border-radius: 0 4px 4px 0;
    }

    hr {
        border: none;
        border-top: 2px solid #e94560;
        margin: 24pt 0;
    }

    ul, ol {
        padding-left: 24pt;
    }

    li {
        margin: 4pt 0;
    }

    strong { color: #16213e; }

    .cover {
        text-align: center;
        page-break-after: always;
        padding-top: 80pt;
    }

    .cover h1 {
        font-size: 32pt;
        border: none;
        color: #1a1a2e;
        margin-top: 0;
    }

    .cover .subtitle {
        font-size: 14pt;
        color: #666;
        margin-top: 20pt;
    }

    .cover .meta {
        font-size: 10pt;
        color: #999;
        margin-top: 40pt;
    }

    a { color: #e94560; text-decoration: none; }
    """

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Zero_Nine 项目全景分析</title>
    <style>{css}</style>
</head>
<body>
    <div class="cover">
        <h1>Zero_Nine</h1>
        <div class="subtitle">项目全景分析文档</div>
        <div class="meta">
            版本 v2.1.0 | 2026-05-17<br>
            Rust Orchestration Kernel | 9 Crate Workspace<br>
            MIT License | Manus AI
        </div>
    </div>
    {body}
</body>
</html>"""

    return html


def html_to_pdf_weasyprint(html_content, output_path):
    """Convert HTML to PDF using WeasyPrint."""
    from weasyprint import HTML, CSS
    HTML(string=html_content).write_pdf(output_path)
    print(f"PDF created: {output_path} (weasyprint)")


def html_to_pdf_cupsfilter(html_path, output_path):
    """Convert HTML to PDF using macOS cupsfilter."""
    # cupsfilter expects PostScript or PDF, not HTML
    # We'll use a different approach: macOS print via osascript
    return False


def html_to_pdf_htmltidy(html_path, output_path):
    """Try using html2ps + ps2pdf."""
    return False


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 md2pdf.py input.md [output.pdf]")
        sys.exit(1)

    md_path = sys.argv[1]
    output_path = sys.argv[2] if len(sys.argv) > 2 else md_path.replace('.md', '.pdf')

    if not os.path.exists(md_path):
        print(f"File not found: {md_path}")
        sys.exit(1)

    print(f"Converting {md_path} to PDF...")

    # Step 1: Convert markdown to HTML
    html_content = markdown_to_html(md_path)

    # Step 2: Try weasyprint
    try:
        html_to_pdf_weasyprint(html_content, output_path)
        return
    except ImportError:
        print("weasyprint not available")
    except Exception as e:
        print(f"weasyprint error: {e}")

    # Step 3: Fallback - save HTML for user to print to PDF
    html_path = output_path.replace('.pdf', '.html')
    with open(html_path, 'w', encoding='utf-8') as f:
        f.write(html_content)
    print(f"\nPDF generation failed. Saved styled HTML instead: {html_path}")
    print("Open this file in Safari and use File > Export as PDF")


if __name__ == '__main__':
    main()
