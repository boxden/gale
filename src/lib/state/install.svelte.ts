import { listen } from '@tauri-apps/api/event';
import type { InstallEvent } from '$lib/types';

/**
 * Shared install-queue activity. A large batch emits `profile_changed` after
 * every installed mod; pages can use this to avoid re-querying their whole
 * dataset between mods while the queue is still running.
 */
class InstallState {
	active = $state(false);

	constructor() {
		void listen<InstallEvent>('install_event', (event) => {
			switch (event.payload.type) {
				case 'show':
					this.active = true;
					break;
				case 'hide':
					this.active = false;
					break;
			}
		});
	}
}

const install = new InstallState();

export default install;
