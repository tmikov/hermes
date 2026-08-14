# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

add_hermes_tool(json-parse-dump
  json-parse-dump.cpp
  LINK_OBJLIBS hermesParser hermesSupport LLVHSupport
  )
