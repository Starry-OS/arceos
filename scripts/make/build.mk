# Main building script

include scripts/make/cargo.mk

ifeq ($(APP_TYPE), c)
  include scripts/make/build_c.mk
else
  rust_package := $(shell cat $(APP)/Cargo.toml | sed -n 's/^name = "\([a-z0-9A-Z_\-]*\)"/\1/p')
  rust_elf := $(TARGET_DIR)/$(TARGET)/$(MODE)/$(rust_package)
endif

ifneq ($(filter $(MAKECMDGOALS),doc doc_check_missing),)
  # run `make doc`
  $(if $(V), $(info RUSTFLAGS: "$(RUSTFLAGS)") $(info RUSTDOCFLAGS: "$(RUSTDOCFLAGS)"))
  export RUSTFLAGS
  export RUSTDOCFLAGS
else ifneq ($(filter $(MAKECMDGOALS),unittest unittest_no_fail_fast),)
  # run `make unittest`
  $(if $(V), $(info RUSTFLAGS: "$(RUSTFLAGS)"))
  export RUSTFLAGS
else ifneq ($(filter $(or $(MAKECMDGOALS), $(.DEFAULT_GOAL)), all build run justrun debug),)
  # run `make build` and other above goals
  ifneq ($(V),)
    $(info APP: "$(APP)")
    $(info APP_TYPE: "$(APP_TYPE)")
    $(info FEATURES: "$(FEATURES)")
    $(info PLAT_CONFIG: "$(PLAT_CONFIG)")
    $(info arceos features: "$(AX_FEAT)")
    $(info lib features: "$(LIB_FEAT)")
    $(info app features: "$(APP_FEAT)")
  endif
  ifeq ($(APP_TYPE), c)
    $(if $(V), $(info CFLAGS: "$(CFLAGS)") $(info LDFLAGS: "$(LDFLAGS)"))
  else ifeq ($(APP_TYPE), rust)
    RUSTFLAGS += $(RUSTFLAGS_LINK_ARGS)
    ifeq ($(BACKTRACE), y)
#       RUSTFLAGS += -C force-frame-pointers -C debuginfo=2 -C strip=none
		ifneq ($(ARCH), loongarch64)
	  		RUSTFLAGS += -Cforce-unwind-tables=yes -Cpanic=unwind -Clink-arg=--eh-frame-hdr
		endif
    endif
    ifeq ($(MYPLAT), axplat-loongarch64-2k1000la)
      RUSTFLAGS += -C target-feature=-ual
    endif
  endif
  $(if $(V), $(info RUSTFLAGS: "$(RUSTFLAGS)"))
  export RUSTFLAGS
  ifeq ($(LTO), y)
    export CARGO_PROFILE_RELEASE_LTO=true
    export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
  endif
endif

_cargo_build: oldconfig
	@printf "    $(GREEN_C)Building$(END_C) App: $(APP_NAME), Arch: $(ARCH), Platform: $(PLAT_NAME), App type: $(APP_TYPE)\n"
ifeq ($(APP_TYPE), rust)
	$(call cargo_build,$(APP),$(AX_FEAT) $(LIB_FEAT) $(APP_FEAT))
	@cp $(rust_elf) $(OUT_ELF)
else ifeq ($(APP_TYPE), c)
	$(call cargo_build,ulib/axlibc,$(AX_FEAT) $(LIB_FEAT))
endif
ifeq ($(BACKTRACE), y)
# 	$(call run_cmd,./scripts/make/dwarf.sh,$(OUT_ELF) $(OBJCOPY))
endif

$(OUT_KSYM): _cargo_build
	@if ! command -v gen_ksym >/dev/null 2>&1; then \
		echo "Installing gen_ksym..."; \
		RUSTFLAGS= cargo install --git https://github.com/Starry-OS/ksym --features=demangle; \
	else \
		echo "gen_ksym already installed."; \
	fi
	@echo "Generating kernel symbols at $@"
	nm -n -C $(OUT_ELF) | grep ' [Tt] ' | grep -v '\.L' | grep -v '$$x' | RUSTFLAGS= gen_ksym > $@

$(OUT_DIR):
	$(call run_cmd,mkdir,-p $@)

$(OUT_BIN): _cargo_build $(OUT_ELF)
	$(call run_cmd,$(OBJCOPY),$(OUT_ELF) -O binary --strip-all $@)
	@if [ ! -s $(OUT_BIN) ]; then \
		echo 'Empty kernel image "$(notdir $(FINAL_IMG))" is built, please check your build configuration'; \
		exit 1; \
	fi

ifeq ($(ARCH), aarch64)
  uimg_arch := arm64
else ifeq ($(ARCH), riscv64)
  uimg_arch := riscv
else
  uimg_arch := $(ARCH)
endif

$(OUT_UIMG): $(OUT_BIN)
	$(call run_cmd,mkimage,\
		-A $(uimg_arch) -O linux -T kernel -C none \
		-a $(subst _,,$(shell axconfig-gen "$(OUT_CONFIG)" -r plat.kernel-base-paddr)) \
		-d $(OUT_BIN) $@)

.PHONY: _cargo_build
