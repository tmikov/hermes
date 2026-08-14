# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

add_hermes_tool(preparse-dump
  preparse-dump.cpp
  LINK_OBJLIBS hermesAST hermesParser hermesSupport LLVHSupport
  )
