// www.noughtpad.com -> https://noughtpad.com, permanently, keeping the path and query.
export default {
  fetch(request) {
    const url = new URL(request.url);
    url.protocol = "https:";
    url.hostname = "noughtpad.com";
    url.port = "";
    return Response.redirect(url.href, 301);
  },
};
