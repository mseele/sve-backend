package de.sve.backend.api;

import java.util.List;

import javax.ws.rs.GET;
import javax.ws.rs.Path;
import javax.ws.rs.Produces;
import javax.ws.rs.core.MediaType;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.api.utils.BackendException;
import de.sve.backend.manager.CalendarManager;
import de.sve.backend.model.calendar.Appointment;

@Path("/calendar")
@SuppressWarnings("static-method")
public class Calendar {

	private static final Logger LOG = LoggerFactory.getLogger(Calendar.class);

	@Path("/appointments")
	@GET
	@Produces(MediaType.APPLICATION_JSON)
	public List<Appointment> appointments() throws BackendException {
		try {
			return CalendarManager.appointments();
		} catch (Throwable t) {
			String message = "Error while loading apointments"; //$NON-NLS-1$
			LOG.error(message, t);
			throw new BackendException(message, t);
		}
	}

}
