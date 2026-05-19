import { convertFileSrc, invoke } from "@tauri-apps/api/core";

const SKIN_ASSETS = [
  "BALANCE.BMP",
  "CBUTTONS.BMP",
  "CLOSE.CUR",
  "EQSLID.CUR",
  "MAIN.BMP",
  "MAINMENU.CUR",
  "MONOSTER.BMP",
  "NUMBERS.BMP",
  "PLAYPAUS.BMP",
  "PLEDIT.BMP",
  "POSBAR.BMP",
  "SHUFREP.BMP",
  "TEXT.BMP",
  "TITLEBAR.BMP",
  "TITLEBAR.CUR",
  "VOLBAL.CUR",
  "VOLUME.BMP",
];

/**
 * @typedef {{ name: string, path: string | null }} SkinAsset
 * @typedef {{ id: string, name: string, bundled: boolean, assets: SkinAsset[] }} BackendSkinInfo
 * @typedef {{ current_skin_id: string, skins_dir: string, skins: BackendSkinInfo[] }} BackendSkinLibrary
 * @typedef {{ id: string, name: string, bundled: boolean, assets: Map<string, string | null> }} SkinInfo
 */

/**
 * @param {string} assetName
 */
function cssVariableName(assetName) {
  return `--skin-${assetName.toLowerCase().replace(/[^a-z0-9]/g, "-")}`;
}

/**
 * @param {string} url
 */
function cssUrl(url) {
  return `url("${url.replace(/"/g, '\\"')}")`;
}

/**
 * @param {BackendSkinInfo} skin
 * @returns {SkinInfo}
 */
function normalizeSkin(skin) {
  /** @type {Map<string, string | null>} */
  const assets = new Map();
  for (const asset of skin.assets) {
    assets.set(asset.name, asset.path ? convertFileSrc(asset.path) : null);
  }

  return { ...skin, assets };
}

class SkinLibrary {
  /** @type {SkinInfo[]} */
  skins = $state([]);
  currentSkinId = $state("base-2.91");
  skinsDir = $state("");

  async load() {
    const library = /** @type {BackendSkinLibrary} */ (
      await invoke("get_skin_library")
    );
    this.setLibrary(library);
  }

  /**
   * @param {BackendSkinLibrary} library
   */
  setLibrary(library) {
    this.currentSkinId = library.current_skin_id;
    this.skinsDir = library.skins_dir;
    this.skins = library.skins.map(normalizeSkin);
    this.applyToDocument();
  }

  /**
   * @returns {SkinInfo | undefined}
   */
  currentSkin() {
    return (
      this.skins.find((skin) => skin.id === this.currentSkinId) ??
      this.skins.find((skin) => skin.id === "base-2.91")
    );
  }

  /**
   * @param {string} assetName
   * @returns {string | null}
   */
  getAssetUrl(assetName) {
    const skin = this.currentSkin();
    return skin?.assets.get(assetName) ?? null;
  }

  applyToDocument() {
    if (!globalThis.document) {
      return;
    }

    const root = document.documentElement;
    for (const assetName of SKIN_ASSETS) {
      const variableName = cssVariableName(assetName);
      const assetUrl = this.getAssetUrl(assetName);
      if (assetUrl) {
        root.style.setProperty(variableName, cssUrl(assetUrl));
      } else {
        root.style.removeProperty(variableName);
      }
    }
  }
}

export const SKIN_LIBRARY = new SkinLibrary();
