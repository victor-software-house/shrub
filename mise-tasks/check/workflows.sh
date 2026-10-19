#!/usr/bin/env bash
#MISE description="Workflow security audit (zizmor, offline)"
# zizmor's offline audits: no network, so the result depends only on the tree.
zizmor --offline .github
