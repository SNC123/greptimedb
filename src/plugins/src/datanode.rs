// Copyright 2023 Greptime Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use common_base::Plugins;
use datanode::config::DatanodeOptions;
use datanode::error::Result;

use crate::options::PluginOptions;

#[allow(unused_variables)]
#[allow(unused_mut)]
pub async fn setup_datanode_plugins(
    plugins: &mut Plugins,
    plugin_options: &[PluginOptions],
    dn_opts: &DatanodeOptions,
) -> Result<()> {
    Ok(())
}

pub async fn start_datanode_plugins(_plugins: Plugins) -> Result<()> {
    Ok(())
}
