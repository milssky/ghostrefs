"""Sphinx configuration for the ghostrefs user guide."""

import os
import tomllib
from pathlib import Path

DOCS = Path(__file__).resolve().parent
PACKAGE = tomllib.loads((DOCS.parent / "Cargo.toml").read_text())["package"]

project = PACKAGE["name"]
author = "ghostrefs contributors"
copyright = "2026, ghostrefs contributors"
version = release = PACKAGE["version"]
language = "en"
root_doc = "index"
nitpicky = True
exclude_patterns = ["_build", "README.md", "requirements.txt", ".DS_Store"]

extensions = [
    "sphinx.ext.githubpages",
    "sphinx_design",
    "sphinx_copybutton",
    "sphinx_tabs.tabs",
    "sphinx_llm_friendly",
]

html_theme = "shibuya"
html_theme_options = {
    "github_url": PACKAGE["repository"],
    "accent_color": "cyan",
    "globaltoc_expand_depth": 1,
    "show_ai_links": False,
}
# Keep local builds usable when the brand asset has not been copied yet.
_logo = "_static/images/ghostrefs-transparent.png"
if (DOCS / _logo).is_file():
    html_theme_options.update(light_logo=_logo, dark_logo=_logo)
    html_favicon = _logo

html_context = {
    "source_type": "github",
    "source_user": "milssky",
    "source_repo": "ghostrefs",
    "source_version": "master",
    "source_docs_path": "/docs/",
}
html_static_path = ["_static"]
html_css_files = ["css/custom.css"]
html_js_files = ["js/sidebar-scroll.js"]
html_show_sourcelink = False
html_sourcelink_suffix = ""
html_baseurl = os.environ.get(
    "READTHEDOCS_CANONICAL_URL", "https://ghostrefs.readthedocs.io/en/latest/"
)
html_title = f"ghostrefs {release} documentation"
copybutton_prompt_text = r"\$ "
copybutton_prompt_is_regexp = True
llm_friendly_llms_txt_summary = (
    f"Documentation for ghostrefs {release}: find code kept alive only by tests. "
    "The complete guide is available in llms-full.txt."
)
llm_friendly_llms_full_txt_max_tokens = 30_000
