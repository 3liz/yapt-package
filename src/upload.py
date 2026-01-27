#
# Upload archive to QGIS xml rpc plugin registry
#
import os
import sys
import base64
import xmlrpc.client


if __name__ == "__main__":

    argv = sys.argv[1:]
    if len(argv) < 2:
        raise ValueError("Missing arguments")

    server_url, auth, archive, *_ = argv

    dry_run = os.getenv("XML_RPC_DRY_RUN")
    verbose = os.getenv("XML_RPC_LOG") == "debug"

    encoded_auth_string = base64.b64encode(auth.encode()).decode()
    server = xmlrpc.client.ServerProxy(
        server_url,
        verbose=os.getenv("XML_RPC_LOG") == "debug",
        headers=[("Authorization", f"Basic {encoded_auth_string}")]
    )

    if dry_run:
        print("Not uploading because dry-run", file=sys.stderr, flush=True)
        with open(archive, "rb") as _: # Test that archive is ok
            pass
    else:
        with open(archive, "rb") as fh:
            try:
                plugin_id, version_id = server.plugin.upload(
                    xmlrpc.client.Binary(fh.read())
                )
                if verbose:
                    print(
                        f"Plugin ID: {plugin_id!r} -- Version ID: {version_id!r}", 
                        file=sys.stderr,
                        flush=True,
                    )
            except  xmlrpc.client.ProtocolError as err:
                import re
                # Hide credentials
                url = re.sub(r":[^/].*@", ":******@", err.url)
                print(
                    "XML RPC Protocol error:\n",
                    f"URL: {url}\n",
                    f"Headers: {err.headers}\n",
                    f"Error code: {err.errcode}\n",
                    f"Error msg: {err.errmsg}\n",
                    f"Plugin path: {archive}",
                    file=sys.stderr,
                    flush=True,
                )
                sys.exit(1)
            except  xmlrpc.client.Fault as err:
                print(
                    "XML RPC Fault:\n",
                    f"Fault code: {err.faultCode}\n",
                    f"Fault string: {err.faultString}\n",
                    f"Plugin path: {archive}",
                    file=sys.stderr,
                    flush=True,
                )
                sys.exit(1)
            
