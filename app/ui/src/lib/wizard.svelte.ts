import { appearance, updateAppearance } from './appearance.svelte';
import { showWizard } from './wizard';

export const wizard = $state({ reopened: false });

export const wizardShown = () => showWizard(appearance.value.setup_done, wizard.reopened);

export function openWizard() {
  wizard.reopened = true;
}

export async function closeWizard() {
  wizard.reopened = false;
  if (!appearance.value.setup_done) await updateAppearance({ setup_done: true }, { replaySplash: false });
}
