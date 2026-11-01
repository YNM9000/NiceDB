<script lang="ts">
  import { m } from '../paraglide/messages';
  import type { HopForm } from '../lib/profile-form';
  import { portOk } from '../lib/profile-form';
  import type { VarPreviewer } from '../lib/vars-preview.svelte';
  import Segmented from './Segmented.svelte';
  import VarHint from './VarHint.svelte';

  let {
    hop = $bindable(),
    preview,
    showErrors,
    hostLabel,
    savedSecret,
  }: { hop: HopForm; preview: VarPreviewer; showErrors: boolean; hostLabel: string; savedSecret: boolean } = $props();

  const sshOptions = (['agent', 'key', 'password'] as const).map((value) => ({
    value,
    label: { agent: m.ssh_agent, key: m.ssh_key, password: m.ssh_password }[value](),
  }));
</script>

<div class="form-row host">
  <label class="form-field" class:invalid={showErrors && !hop.host.trim()}>
    <span class="caps">{hostLabel}</span>
    <span class="field"><input bind:value={hop.host} placeholder={m.form_ph_ssh_host()} spellcheck="false" autocomplete="off" /></span>
    <VarHint text={hop.host} {preview} />
  </label>
  <label class="form-field" class:invalid={showErrors && !portOk(hop.port)}>
    <span class="caps">{m.form_port()}</span>
    <span class="field"><input bind:value={hop.port} inputmode="numeric" autocomplete="off" /></span>
    <VarHint text={hop.port} {preview} />
  </label>
</div>
<div class="form-row two">
  <label class="form-field" class:invalid={showErrors && !hop.user.trim()}>
    <span class="caps">{m.form_ssh_user()}</span>
    <span class="field"><input bind:value={hop.user} spellcheck="false" autocomplete="off" /></span>
    <VarHint text={hop.user} {preview} />
  </label>
  <div class="form-field">
    <span class="caps">{m.form_ssh_auth()}</span>
    <Segmented label={m.form_ssh_auth()} value={hop.method} options={sshOptions} onchange={(v) => (hop.method = v)} />
  </div>
</div>
{#if hop.method === 'key'}
  <div class="form-row two">
    <label class="form-field">
      <span class="caps">{m.form_ssh_key()}</span>
      <span class="field"><input bind:value={hop.key} spellcheck="false" autocomplete="off" /></span>
      <VarHint text={hop.key} {preview} />
    </label>
    <label class="form-field">
      <span class="caps">{m.form_ssh_passphrase()}</span>
      <span class="field"><input type="password" bind:value={hop.secret} placeholder={savedSecret ? m.form_password_saved() : ''} autocomplete="new-password" /></span>
    </label>
  </div>
{:else if hop.method === 'password'}
  <label class="form-field">
    <span class="caps">{m.form_ssh_password()}</span>
    <span class="field"><input type="password" bind:value={hop.secret} placeholder={savedSecret ? m.form_password_saved() : ''} autocomplete="new-password" /></span>
  </label>
{/if}
