# Link Shortner

A stupidly fast in-memory link shortner.

### Docker

```
docker build -t short-link .
docker run -p 5008:5008 --name short-link short-link
```

### Traefik CORS
```
traefik.http.routers.https-0-tw00w88.middlewares=cors
traefik.http.middlewares.cors.headers.accesscontrolallowmethods=*
traefik.http.middlewares.cors.headers.accesscontrolalloworiginlist=*
traefik.http.middlewares.cors.headers.accesscontrolallowheaders=*
traefik.http.middlewares.cors.headers.accesscontrolmaxage=100
traefik.http.middlewares.cors.headers.addvaryheader=true
```