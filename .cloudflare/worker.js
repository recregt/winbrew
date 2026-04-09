export default {
  async fetch(request) {
    const url = new URL(request.url);

    if (url.pathname.startsWith("/doc")) {
      const newPath = url.pathname.slice("/doc".length) || "/";
      const newUrl = "https://winbrew.pages.dev" + newPath + url.search;
      return fetch(newUrl);
    }

    if (url.pathname.startsWith("/winbrew")) {
      const newUrl = "https://winbrew.pages.dev" + url.pathname + url.search;
      return fetch(newUrl);
    }

    return new Response("Coming soon", { status: 200 });
  }
}