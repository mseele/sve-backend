package de.sve.backend.api.utils;

import jakarta.ws.rs.core.Response;
import jakarta.ws.rs.core.Response.Status;
import jakarta.ws.rs.ext.ExceptionMapper;
import jakarta.ws.rs.ext.Provider;

@Provider
public class BackendExceptionHandler implements ExceptionMapper<BackendException> {

	@Override
	public Response toResponse(BackendException exception) {
		return Response.status(Status.BAD_REQUEST).entity(exception).build();
	}

}