SHELL:=bash

NAME=$(shell cargo metadata --no-deps --format-version 1 | jq -r .packages[0].name)
VERSION ?= $(shell cargo metadata --no-deps --format-version 1 | jq -r .packages[0].version)

echo-%: 
	@cargo metadata --no-deps --format-version 1 | jq -r .packages[0].$*

PACKAGE=$(shell echo $(NAME)-$(VERSION)-$$(uname -m)-unknown-linux-gnu.tar.gz)

package:
	@if [[ "$$(uname -o)" == "GNU/Linux" ]]; then \
	tar -czf $(PACKAGE) -C ./target/release/ $(NAME); \
	else \
	echo "Can only package for GNU/Linux OS" && false; \
	fi
	@echo $(PACKAGE)


wheel:
	maturin build --release --strip
