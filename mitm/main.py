from mitmproxy import http
import json

# !~u /clip/v2/resource/bridge

def responseheaders(flow: http.HTTPFlow):
    """
    Enables streaming for all responses.
    This is equivalent to passing `--set stream_large_bodies=1` to mitmproxy.
    """
    flow.response.stream = flow.response.headers.get('content-type', '').startswith('text/event-stream')

def response(flow: http.HTTPFlow) -> None:
    flow.response.stream = flow.response.headers.get('content-type', '').startswith('text/event-stream')


    # print('Content type', flow.response.headers.get('content-type', ''), flow.response.headers.get('content-type', '').startswith('text/event-stream'))

    if flow.request.method.lower().strip() == 'get':
        if flow.request.pretty_url.endswith("/config"):
            # /api/QVtu-akopleGPnqEYknZUd4SI1mDkzlQkRSVVk7G/confi
            # print('Raw', flow.request, flow.response.get_text())
            responsedata = json.loads(flow.response.get_text())
            # print(responsedata)
            dict_temp = {
                "bridgeid": "144F8AFFFEA0E9A2",
                "mac": "14:4f:8a:a0:e9:a2",
                "name": "MITM Bridge",
                "ipaddress": "192.168.10.134"
            }

            if 'bridgeid' in responsedata:
                responsedata.update(dict_temp)
            flow.response.text = json.dumps(responsedata)

    if flow.request.pretty_url.endswith("/clip/v2/resource/bridge"):
        responsedata = json.loads(flow.response.get_text())        
        
        responsedata['data'][0]['bridge_id'] = "144F8AFFFEA0E9A2"
        
        flow.response.text = json.dumps(responsedata)

def request(flow: http.HTTPFlow):
    pass
