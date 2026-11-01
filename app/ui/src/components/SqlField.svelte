<script lang="ts">
  import { EditorView } from '@codemirror/view';
  import { onMount } from 'svelte';
  import type { Dialect } from '../lib/bindings';
  import { fieldExtensions } from '../lib/editor';

  interface Props {
    value: string;
    dialect: Dialect | null;
    label: string;
    hint: string;
    onchange: (value: string) => void;
  }
  let { value, dialect, label, hint, onchange }: Props = $props();
  let host: HTMLDivElement;

  onMount(() => {
    const view = new EditorView({ parent: host, doc: value, extensions: fieldExtensions(dialect, label, hint, (v) => onchange(v)) });
    return () => view.destroy();
  });
</script>

<div class="field sql-field" bind:this={host}></div>
