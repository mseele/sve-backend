package de.sve.backend.api.utils;

import java.io.IOException;
import java.util.List;
import java.util.Map.Entry;

import jakarta.ws.rs.container.ContainerRequestContext;
import jakarta.ws.rs.container.ContainerRequestFilter;
import jakarta.ws.rs.container.ContainerResponseContext;
import jakarta.ws.rs.container.ContainerResponseFilter;
import jakarta.ws.rs.container.PreMatching;
import jakarta.ws.rs.core.Response;
import jakarta.ws.rs.ext.Provider;

@Provider
@PreMatching
public class CORSFilter implements ContainerRequestFilter, ContainerResponseFilter {

	private static final String ACCESS_CONTROL_REQUEST_HEADERS = "access-control-request-headers"; //$NON-NLS-1$

	/**
	 * Method for ContainerRequestFilter.
	 */
	@Override
	public void filter(ContainerRequestContext request) throws IOException {
		// If it's a preflight request, we abort the request with
		// a 200 status, and the CORS headers are added in the
		// response filter method below.
		if (isPreflightRequest(request)) {
			request.abortWith(Response.ok().build());
			return;
		}
	}

	/**
	 * A preflight request is an OPTIONS request with an Origin header.
	 */
	private static boolean isPreflightRequest(ContainerRequestContext request) {
		return request.getHeaderString("Origin") != null && request.getMethod().equalsIgnoreCase("OPTIONS"); //$NON-NLS-1$ //$NON-NLS-2$
	}

	/**
	 * Method for ContainerResponseFilter.
	 */
	@Override
	public void filter(ContainerRequestContext request, ContainerResponseContext response) throws IOException {
		// if there is no Origin header, then it is not a
		// cross origin request. We don't do anything.
		if (request.getHeaderString("Origin") == null) { //$NON-NLS-1$
			return;
		}

		// If it is a preflight request, then we add all
		// the CORS headers here.
		if (isPreflightRequest(request)) {
			response.getHeaders().add("Access-Control-Allow-Credentials", "true"); //$NON-NLS-1$ //$NON-NLS-2$
			response.getHeaders().add("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS, HEAD"); //$NON-NLS-1$ //$NON-NLS-2$
			// Allow all requested headers - if there are some
			for (Entry<String, List<String>> entry : request.getHeaders().entrySet()) {
				if (ACCESS_CONTROL_REQUEST_HEADERS.equals(entry.getKey().toLowerCase())) {
					for (String value : entry.getValue()) {
						if (value != null) {
							response.getHeaders().add("Access-Control-Allow-Headers", value); //$NON-NLS-1$
							break;
						}
					}
					break;
				}
			}
		}

		// Cross origin requests can be either simple requests
		// or preflight request. We need to add this header
		// to both type of requests. Only preflight requests
		// need the previously added headers.
		response.getHeaders().add("Access-Control-Allow-Origin", "*"); //$NON-NLS-1$ //$NON-NLS-2$
	}

}