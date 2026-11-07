/** Landing page: tells visitors on other systems that Juice is Windows-only, before they download. */
const platform = document.querySelector<HTMLElement>('#platform');
const download = document.querySelector<HTMLAnchorElement>('#download');
const windows = /Windows/i.test(navigator.userAgent);

if (platform && download && !windows) {
  platform.textContent = 'Juice runs on Windows 10 and 11. Open this page there to download.';
  download.classList.remove('primary');
}
