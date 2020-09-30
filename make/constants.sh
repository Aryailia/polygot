#export PROJECT_DIR="$( dirname "$0"; printf a )"; PROJECT_DIR="${PROJECT_DIR%?a}"
export  CONFIG_DIR="config"  # Website configuration
export    MAKE_DIR="make"    # Builder scripts
export  SOURCE_DIR='src'     # Assests and content source
export  PUBLIC_DIR='public'  # Compiled, public-facing content
export WORKING_DIR='.blog'


export WORKING_BODY_DIR="${WORKING_DIR}/_body"
export WORKING_TOC_DIR="${WORKING_DIR}/_toc"
export BLOG_OUTPUT_DIR="${PUBLIC_DIR}/blog"  # Directory
export TAGS_INDEX="${BLOG_OUTPUT}/tags"  # File
