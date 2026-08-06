# Constants:
TARGET_FOLDER       = target

BOLD   := \033[1m
COLOR_RESET  = $(call get_color,sgr0,)
COLOR_CYAN   = $(call get_color,setaf,6)
COLOR_GREEN  = $(call get_color,setaf,2)
COLOR_YELLOW = $(call get_color,setaf,3)

# Some colors (if supported)
define get_color
$(shell tput -Txterm $(1) $(2) 2>/dev/null || echo "")
endef

.PHONY: all
all: help

## Build:
.PHONY: build
build: build-rust ## Build rust (i.e. not docker)

.PHONY: build-rust
build-rust: ## Build the Rust sources
	@printf "$(COLOR_GREEN)$(BOLD)🔨 Building...$(COLOR_RESET)\n"
	cargo build --all-features

## Help:
.PHONY: help
help: ## Show this help
	@echo ''
	@echo 'Usage:'
	@echo '  ${COLOR_YELLOW}make${COLOR_RESET} ${COLOR_GREEN}<target>${COLOR_RESET}'
	@echo ''
	@echo 'Targets:'
	@$(foreach V,$(sort $(.VARIABLES)), \
		$(if $(filter-out environment% default automatic,$(origin $V)), \
			$(if $(filter TOOL_%,$V), \
				export $V="$($V)";))) \
	awk 'BEGIN {FS = ":.*?## "} { \
		if (/^[a-zA-Z_-]+:.*?##.*$$/) {printf "    ${COLOR_YELLOW}%-20s${COLOR_GREEN}%s${COLOR_RESET}\n", $$1, $$2} \
		else if (/^## .*$$/) {printf "  ${COLOR_CYAN}%s${COLOR_RESET}\n", substr($$1,4)} \
		}' $(MAKEFILE_LIST) | envsubst
