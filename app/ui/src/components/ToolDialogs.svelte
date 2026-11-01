<script lang="ts">
  import { ddlUi } from '../lib/ddl.svelte';
  import { depsUi } from '../lib/sqlgen.svelte';
  import DepsDialog from './DepsDialog.svelte';
  import { toolsUi } from '../lib/tools.svelte';
  import DdlDialog from './DdlDialog.svelte';
  import DataExportDialog from './DataExportDialog.svelte';
  import DataImportDialog from './DataImportDialog.svelte';
  import { dynamoUi } from '../lib/dynamo-builder.svelte';
  import DynamoBuilder from './DynamoBuilder.svelte';
</script>

{#if toolsUi.export}
  {#key toolsUi.export}
    <DataExportDialog ask={toolsUi.export} onclose={() => (toolsUi.export = null)} />
  {/key}
{/if}
{#if toolsUi.import}
  {#key toolsUi.import}
    <DataImportDialog ask={toolsUi.import} onclose={() => (toolsUi.import = null)} />
  {/key}
{/if}
{#if ddlUi.ask}
  {#key ddlUi.ask}
    <DdlDialog ask={ddlUi.ask} onclose={() => (ddlUi.ask = null)} />
  {/key}
{/if}
{#if depsUi.ask}
  {#key depsUi.ask}
    <DepsDialog ask={depsUi.ask} onclose={() => (depsUi.ask = null)} />
  {/key}
{/if}
{#if dynamoUi.ask}
  {#key dynamoUi.ask}
    <DynamoBuilder ask={dynamoUi.ask} onclose={() => (dynamoUi.ask = null)} />
  {/key}
{/if}
